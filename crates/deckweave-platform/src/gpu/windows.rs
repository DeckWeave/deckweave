use super::{Adapter, GpuSample, aggregate};
use ::windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE, DXGI_ERROR_NOT_FOUND, IDXGIFactory1,
};
use std::{mem::size_of, ptr, time::Instant};
use windows_sys::Win32::System::Performance::*;

/// One query for the helper's lifetime. PDH owns the counters; closing the query
/// frees all of them. No external process, registry changes or elevated privileges.
pub struct GpuSampler {
    query: PDH_HQUERY,
    engine: Result<PDH_HCOUNTER, String>,
    memory: Result<PDH_HCOUNTER, String>,
    last_collect: Option<Instant>,
    cached: Option<GpuSample>,
}

fn pdh_error(operation: &str, status: u32) -> String {
    format!("{operation}: PDH 0x{status:08x}; WDDM GPU counters may be unavailable")
}

impl GpuSampler {
    pub fn new() -> Result<Self, String> {
        let mut query = ptr::null_mut();
        // SAFETY: output handle is initialized, data source is local real-time.
        let status = unsafe { PdhOpenQueryW(ptr::null(), 0, &mut query) };
        if status != 0 {
            return Err(pdh_error("Open query", status));
        }
        let add = |path: &str| {
            let path: Vec<u16> = path.encode_utf16().chain([0]).collect();
            let mut counter = ptr::null_mut();
            // SAFETY: query is live; NUL-terminated path lives through the call.
            // English API localizes counter names, independent of OS language.
            let status = unsafe { PdhAddEnglishCounterW(query, path.as_ptr(), 0, &mut counter) };
            if status == 0 {
                Ok(counter)
            } else {
                Err(pdh_error("Add counter", status))
            }
        };
        Ok(Self {
            query,
            engine: add(r"\GPU Engine(*)\Utilization Percentage"),
            memory: add(r"\GPU Adapter Memory(*)\Dedicated Usage"),
            last_collect: None,
            cached: None,
        })
    }

    pub fn sample(&mut self, now: u64) -> GpuSample {
        // Bound native collection even if a caller floods the request protocol.
        if self
            .last_collect
            .is_some_and(|t| t.elapsed().as_millis() < 1000)
            && let Some(cached) = &self.cached
        {
            return cached.clone();
        }
        let adapters = match adapters() {
            Ok(adapters) => adapters,
            Err(reason) => return GpuSample::unavailable(reason, now),
        };
        let warming_up = self.last_collect.is_none();
        // SAFETY: query and all its counters remain owned until Drop.
        let status = unsafe { PdhCollectQueryData(self.query) };
        self.last_collect = Some(Instant::now());
        let read = |counter: &Result<PDH_HCOUNTER, String>| {
            if status != 0 {
                return Err(pdh_error("Collect counters", status));
            }
            match counter {
                Ok(counter) => counter_values(*counter),
                Err(error) => Err(error.clone()),
            }
        };
        let sample = aggregate(
            &adapters,
            read(&self.engine),
            read(&self.memory),
            warming_up,
            now,
        );
        self.cached = Some(sample.clone());
        sample
    }
}

impl Drop for GpuSampler {
    fn drop(&mut self) {
        // SAFETY: this is the one owner; counters are not used after drop.
        unsafe { PdhCloseQuery(self.query) };
    }
}

fn adapters() -> Result<Vec<Adapter>, String> {
    // SAFETY: typed DXGI interfaces own their references. A fresh factory observes
    // adapter changes; no retained COM interface or stale LUID after undocking.
    unsafe {
        let factory: IDXGIFactory1 =
            CreateDXGIFactory1().map_err(|e| format!("DXGI adapter discovery unavailable: {e}"))?;
        let mut result = vec![];
        for index in 0..16 {
            let adapter = match factory.EnumAdapters1(index) {
                Ok(adapter) => adapter,
                Err(error) if error.code() == DXGI_ERROR_NOT_FOUND => break,
                Err(error) => return Err(format!("DXGI adapter discovery failed: {error}")),
            };
            let desc = adapter
                .GetDesc1()
                .map_err(|e| format!("DXGI adapter identity failed: {e}"))?;
            if desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32 != 0 {
                continue;
            }
            let length = desc
                .Description
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(desc.Description.len());
            result.push(Adapter {
                id: format!(
                    "{:08x}:{:08x}",
                    desc.AdapterLuid.HighPart as u32, desc.AdapterLuid.LowPart
                ),
                index,
                name: String::from_utf16_lossy(&desc.Description[..length]),
                dedicated_total_bytes: desc.DedicatedVideoMemory as u64,
            });
        }
        Ok(result)
    }
}

fn counter_values(counter: PDH_HCOUNTER) -> Result<Vec<(String, f64)>, String> {
    const LIMIT: usize = 4 * 1024 * 1024;
    // PDH_FMT_NOCAP100 is documented by PDH but absent from windows-sys bindings.
    const FORMAT: u32 = PDH_FMT_DOUBLE | 0x8000;
    let mut bytes = 0;
    let mut count = 0;
    // SAFETY: null buffer with zero length requests required allocation size.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(counter, FORMAT, &mut bytes, &mut count, ptr::null_mut())
    };
    if status != PDH_MORE_DATA {
        return Err(pdh_error("Read counter size", status));
    }
    if bytes as usize > LIMIT || bytes == 0 {
        return Err("GPU counter response exceeds limit or is empty".into());
    }
    // u64 storage gives sufficient alignment for the native item array and strings.
    let mut buffer = vec![0_u64; (bytes as usize).div_ceil(size_of::<u64>())];
    let capacity = buffer.len() * size_of::<u64>();
    // SAFETY: buffer is aligned, writable and sized from PDH's first call. If the
    // dynamic instance set grows, PDH returns an error and we retry next sample.
    let status = unsafe {
        PdhGetFormattedCounterArrayW(
            counter,
            FORMAT,
            &mut bytes,
            &mut count,
            buffer.as_mut_ptr().cast(),
        )
    };
    if status != 0 {
        return Err(pdh_error("Read counters", status));
    }
    if count as usize > 16_384
        || count as usize * size_of::<PDH_FMT_COUNTERVALUE_ITEM_W>() > capacity
    {
        return Err("GPU counter item count exceeds limit".into());
    }
    let start = buffer.as_ptr() as usize;
    let end = start + capacity;
    // SAFETY: validated item count fits allocation; PDH initialized the array.
    let items = unsafe {
        std::slice::from_raw_parts(
            buffer.as_ptr().cast::<PDH_FMT_COUNTERVALUE_ITEM_W>(),
            count as usize,
        )
    };
    let mut result = Vec::with_capacity(items.len());
    for item in items {
        if ![PDH_CSTATUS_VALID_DATA, PDH_CSTATUS_NEW_DATA].contains(&item.FmtValue.CStatus) {
            continue;
        }
        let name = item.szName as usize;
        if name < start || name >= end || !name.is_multiple_of(2) {
            continue;
        }
        let max_chars = ((end - name) / 2).min(2048);
        // SAFETY: string pointer and maximum extent were checked within the live
        // allocation; a missing terminator is rejected without reading past it.
        let chars = unsafe { std::slice::from_raw_parts(item.szName, max_chars) };
        let Some(length) = chars.iter().position(|c| *c == 0) else {
            continue;
        };
        // SAFETY: PDH_FMT_DOUBLE selected this union field; CStatus is valid.
        let value = unsafe { item.FmtValue.Anonymous.doubleValue };
        result.push((String::from_utf16_lossy(&chars[..length]), value));
    }
    Ok(result)
}
