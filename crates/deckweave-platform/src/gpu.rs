//! Read-only whole-adapter telemetry. Per-process engine counters are summed by
//! physical engine before taking the busiest engine; memory uses adapter counters,
//! never the known-problematic sum of per-process dedicated allocations.
use serde::Serialize;
use std::collections::BTreeMap;

#[cfg(windows)]
#[allow(unsafe_code)]
mod windows;
#[cfg(windows)]
pub use windows::GpuSampler;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SampleStatus {
    Ready,
    WarmingUp,
    Unavailable,
}

#[derive(Clone, Debug, Serialize)]
pub struct AdapterSample {
    pub id: String,
    pub index: u32,
    pub name: String,
    pub dedicated_total_bytes: u64,
    pub dedicated_used_bytes: Option<u64>,
    pub busiest_engine_percent: Option<f64>,
    pub gpu_status: SampleStatus,
    pub memory_status: SampleStatus,
    pub gpu_reason: Option<String>,
    pub memory_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct GpuSample {
    pub status: SampleStatus,
    pub sampled_at_ms: u64,
    pub adapters: Vec<AdapterSample>,
    pub selected_adapter_id: Option<String>,
    pub reason: Option<String>,
}

impl GpuSample {
    pub fn unavailable(reason: String, now: u64) -> Self {
        Self {
            status: SampleStatus::Unavailable,
            sampled_at_ms: now,
            adapters: vec![],
            selected_adapter_id: None,
            reason: Some(reason),
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Adapter {
    pub id: String,
    pub index: u32,
    pub name: String,
    pub dedicated_total_bytes: u64,
}

/// The OS-defined instance spelling contains the adapter LUID (high then low),
/// physical device and engine index. Ignore PID and engine-type labels: multiple
/// processes on one engine must add together, different engines must not.
fn instance_identity(instance: &str) -> Option<(String, u32, Option<u32>)> {
    let fields: Vec<_> = instance.split('_').collect();
    let luid = fields.iter().position(|s| *s == "luid")?;
    let high = u32::from_str_radix(fields.get(luid + 1)?.strip_prefix("0x")?, 16).ok()?;
    let low = u32::from_str_radix(fields.get(luid + 2)?.strip_prefix("0x")?, 16).ok()?;
    let physical = fields.iter().position(|s| *s == "phys")?;
    let physical = fields.get(physical + 1)?.parse().ok()?;
    let engine = fields
        .iter()
        .position(|s| *s == "eng")
        .and_then(|p| fields.get(p + 1)?.parse().ok());
    Some((format!("{high:08x}:{low:08x}"), physical, engine))
}

pub(super) fn aggregate(
    adapters: &[Adapter],
    engines: Result<Vec<(String, f64)>, String>,
    memory: Result<Vec<(String, f64)>, String>,
    warming_up: bool,
    now: u64,
) -> GpuSample {
    let mut engine_totals = BTreeMap::<(String, u32, u32), f64>::new();
    if let Ok(items) = &engines {
        for (instance, value) in items {
            if value.is_finite()
                && *value >= 0.0
                && let Some((id, physical, Some(engine))) = instance_identity(instance)
            {
                *engine_totals.entry((id, physical, engine)).or_default() += value;
            }
        }
    }
    let mut memory_totals = BTreeMap::<String, u64>::new();
    if let Ok(items) = &memory {
        for (instance, value) in items {
            if value.is_finite() && *value >= 0.0 && *value <= u64::MAX as f64
                && let Some((id, _, None)) = instance_identity(instance)
                // GPU Adapter Memory has one instance per physical adapter. PID
                // instances belong to GPU Process Memory and must never enter here.
                && !instance.starts_with("pid_")
            {
                let entry = memory_totals.entry(id).or_default();
                *entry = entry.saturating_add(*value as u64);
            }
        }
    }
    let adapters: Vec<_> = adapters
        .iter()
        .map(|adapter| {
            let load = engine_totals
                .iter()
                .filter(|((id, _, _), _)| id == &adapter.id)
                .map(|(_, value)| value.clamp(0.0, 100.0))
                .reduce(f64::max);
            let used = memory_totals.get(&adapter.id).copied();
            let gpu_status = if warming_up {
                SampleStatus::WarmingUp
            } else if load.is_some() {
                SampleStatus::Ready
            } else {
                SampleStatus::Unavailable
            };
            let memory_status = if adapter.dedicated_total_bytes == 0 || used.is_none() {
                SampleStatus::Unavailable
            } else {
                SampleStatus::Ready
            };
            AdapterSample {
                id: adapter.id.clone(),
                index: adapter.index,
                name: adapter.name.clone(),
                dedicated_total_bytes: adapter.dedicated_total_bytes,
                dedicated_used_bytes: used,
                busiest_engine_percent: if warming_up { None } else { load },
                gpu_reason: match gpu_status {
                    SampleStatus::WarmingUp => Some("Waiting for a second counter sample".into()),
                    SampleStatus::Unavailable => {
                        Some(engines.as_ref().err().cloned().unwrap_or_else(|| {
                            "No valid GPU engine counters for this adapter".into()
                        }))
                    }
                    SampleStatus::Ready => None,
                },
                memory_reason: if adapter.dedicated_total_bytes == 0 {
                    Some("Adapter has no dedicated video memory".into())
                } else if used.is_none() {
                    Some(
                        memory
                            .as_ref()
                            .err()
                            .cloned()
                            .unwrap_or_else(|| "No valid adapter memory counter".into()),
                    )
                } else {
                    None
                },
                gpu_status,
                memory_status,
            }
        })
        .collect();
    // Default to the adapter whose busiest engine is currently highest. The same
    // adapter is used for GPU and VRAM; explicit UI selection overrides this.
    let selected = adapters
        .iter()
        .filter(|a| a.busiest_engine_percent.is_some())
        .max_by(|a, b| {
            a.busiest_engine_percent
                .partial_cmp(&b.busiest_engine_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| b.index.cmp(&a.index))
        })
        .or_else(|| adapters.first());
    let selected_adapter_id = selected.map(|a| a.id.clone());
    GpuSample {
        status: if adapters.is_empty() {
            SampleStatus::Unavailable
        } else if warming_up {
            SampleStatus::WarmingUp
        } else if adapters
            .iter()
            .any(|a| a.gpu_status == SampleStatus::Ready || a.memory_status == SampleStatus::Ready)
        {
            SampleStatus::Ready
        } else {
            SampleStatus::Unavailable
        },
        sampled_at_ms: now,
        selected_adapter_id,
        reason: adapters
            .is_empty()
            .then(|| "No hardware DXGI adapters found".into()),
        adapters,
    }
}

#[cfg(not(windows))]
pub struct GpuSampler;
#[cfg(not(windows))]
impl GpuSampler {
    pub fn new() -> Result<Self, String> {
        Err("GPU telemetry requires Windows WDDM 2 or later".into())
    }
    pub fn sample(&mut self, now: u64) -> GpuSample {
        GpuSample::unavailable("GPU telemetry requires Windows".into(), now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn adapters() -> Vec<Adapter> {
        vec![
            Adapter {
                id: "00000000:00001234".into(),
                index: 0,
                name: "Fixture GPU".into(),
                dedicated_total_bytes: 8 << 30,
            },
            Adapter {
                id: "00000001:00001234".into(),
                index: 1,
                name: "Other GPU".into(),
                dedicated_total_bytes: 0,
            },
        ]
    }
    fn engine(pid: u32, engine: u32, value: f64) -> (String, f64) {
        (
            format!("pid_{pid}_luid_0x00000000_0x00001234_phys_0_eng_{engine}_engtype_3D"),
            value,
        )
    }
    #[test]
    fn parses_luids_and_rejects_malformed_or_missing_identity() {
        assert_eq!(
            instance_identity(&engine(1, 2, 0.0).0),
            Some(("00000000:00001234".into(), 0, Some(2)))
        );
        assert!(instance_identity("luid_0x0_garbage_phys_0").is_none());
        assert!(instance_identity("_Total").is_none());
    }
    #[test]
    fn sums_processes_per_engine_then_uses_busiest_not_sum_of_engines() {
        let result = aggregate(
            &adapters(),
            Ok(vec![
                engine(1, 0, 30.0),
                engine(2, 0, 40.0),
                engine(3, 1, 60.0),
                engine(4, 2, f64::NAN),
            ]),
            Ok(vec![("luid_0x00000000_0x00001234_phys_0".into(), 1234.0)]),
            false,
            5,
        );
        assert_eq!(result.adapters[0].busiest_engine_percent, Some(70.0));
        assert_eq!(result.adapters[0].dedicated_used_bytes, Some(1234));
        assert_eq!(result.adapters[1].busiest_engine_percent, None);
        assert_eq!(result.adapters[1].memory_status, SampleStatus::Unavailable);
    }
    #[test]
    fn real_zero_is_ready_and_warmup_and_missing_data_never_become_zero() {
        let ready = aggregate(
            &adapters(),
            Ok(vec![engine(1, 0, 0.0)]),
            Ok(vec![("luid_0x00000000_0x00001234_phys_0".into(), 0.0)]),
            false,
            5,
        );
        assert_eq!(ready.adapters[0].gpu_status, SampleStatus::Ready);
        assert_eq!(ready.adapters[0].dedicated_used_bytes, Some(0));
        let warm = aggregate(
            &adapters(),
            Ok(vec![engine(1, 0, 0.0)]),
            Ok(vec![]),
            true,
            5,
        );
        assert_eq!(warm.adapters[0].gpu_status, SampleStatus::WarmingUp);
        assert_eq!(warm.adapters[0].busiest_engine_percent, None);
        let missing = aggregate(
            &adapters(),
            Err("PDH unavailable".into()),
            Ok(vec![]),
            false,
            5,
        );
        assert_eq!(missing.adapters[0].gpu_status, SampleStatus::Unavailable);
        assert_eq!(
            missing.adapters[0].gpu_reason.as_deref(),
            Some("PDH unavailable")
        );
    }
    #[test]
    fn selects_busiest_adapter_and_sums_physical_memory_without_crossing_luids() {
        let second = engine(2, 0, 80.0)
            .0
            .replace("luid_0x00000000", "luid_0x00000001");
        let result = aggregate(
            &adapters(),
            Ok(vec![engine(1, 0, 10.0), (second, 80.0)]),
            Ok(vec![
                ("luid_0x00000000_0x00001234_phys_0".into(), 100.0),
                ("luid_0x00000000_0x00001234_phys_1".into(), 200.0),
                ("luid_0x00000001_0x00001234_phys_0".into(), 500.0),
            ]),
            false,
            5,
        );
        assert_eq!(
            result.selected_adapter_id.as_deref(),
            Some("00000001:00001234")
        );
        assert_eq!(result.adapters[0].dedicated_used_bytes, Some(300));
        assert_eq!(result.adapters[1].dedicated_used_bytes, Some(500));
        assert_eq!(result.adapters[0].busiest_engine_percent, Some(10.0));
    }
    #[test]
    fn never_sums_process_memory_and_clamps_only_engine_load() {
        let result = aggregate(
            &adapters(),
            Ok(vec![engine(1, 0, 150.0)]),
            Ok(vec![(
                "pid_1_luid_0x00000000_0x00001234_phys_0".into(),
                9999.0,
            )]),
            false,
            5,
        );
        assert_eq!(result.adapters[0].busiest_engine_percent, Some(100.0));
        assert_eq!(result.adapters[0].dedicated_used_bytes, None);
        assert!(
            aggregate(&[], Ok(vec![]), Ok(vec![]), false, 5)
                .selected_adapter_id
                .is_none()
        );
    }
}
