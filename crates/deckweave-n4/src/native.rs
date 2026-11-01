use crate::packets;
use hidapi::{DeviceInfo, HidApi, HidDevice};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    pub identity: String,
    pub product_id: u16,
    pub identity_source: &'static str,
}
fn supported(info: &DeviceInfo) -> bool {
    info.vendor_id() == 0x5548
        && [0x1008, 0x1021].contains(&info.product_id())
        && info.interface_number() == 0
        && info.usage_page() == 0xffa0
        && info.usage() == 1
}
fn identity(info: &DeviceInfo) -> Candidate {
    let serial = info.serial_number().filter(|s| !s.trim().is_empty());
    let source = serial
        .map(|s| format!("{:04x}:{:04x}:{s}", info.vendor_id(), info.product_id()))
        .unwrap_or_else(|| info.path().to_string_lossy().into_owned());
    Candidate {
        identity: format!("n4pro-{:x}", Sha256::digest(source.as_bytes())),
        product_id: info.product_id(),
        identity_source: if serial.is_some() {
            "usb_serial"
        } else {
            "usb_path"
        },
    }
}
pub fn discover() -> Result<Vec<Candidate>, String> {
    let api = HidApi::new().map_err(|e| e.to_string())?;
    let mut seen = BTreeSet::new();
    let mut found = vec![];
    for info in api.device_list().filter(|i| supported(i)) {
        let candidate = identity(info);
        if !seen.insert(candidate.identity.clone()) {
            return Err("Ambiguous N4 identity; refusing to choose a device".into());
        }
        found.push(candidate);
        if found.len() > 16 {
            return Err("N4 instance limit exceeded".into());
        }
    }
    Ok(found)
}
pub struct Connection(HidDevice);
#[derive(Debug)]
pub enum OpenError {
    Offline,
    Failed(String),
}
impl From<String> for OpenError {
    fn from(message: String) -> Self {
        Self::Failed(message)
    }
}
impl Connection {
    pub fn open(wanted: &str) -> Result<Self, OpenError> {
        let api = HidApi::new().map_err(|e| e.to_string())?;
        let mut matches = api
            .device_list()
            .filter(|i| supported(i) && identity(i).identity == wanted);
        let info = matches.next().ok_or(OpenError::Offline)?;
        if matches.next().is_some() {
            return Err(OpenError::Failed("Ambiguous N4 identity".into()));
        }
        let connection = Self(info.open_device(&api).map_err(|e| e.to_string())?);
        let mut buffer = [0; 1024];
        let mut drained = false;
        for _ in 0..512 {
            if connection
                .0
                .read_timeout(&mut buffer, 0)
                .map_err(|e| e.to_string())?
                == 0
            {
                drained = true;
                break;
            }
        }
        if !drained {
            return Err(OpenError::Failed("N4 has an input backlog".into()));
        }
        connection.write(&packets::wake())?;
        connection.write(&packets::heartbeat())?;
        connection.write(&packets::clear_background())?;
        Ok(connection)
    }
    pub fn read(&self, buffer: &mut [u8]) -> Result<usize, String> {
        self.0.read_timeout(buffer, 25).map_err(|e| e.to_string())
    }
    pub fn write(&self, report: &[u8]) -> Result<(), String> {
        let count = self.0.write(report).map_err(|e| e.to_string())?;
        if count != report.len() {
            return Err("Incomplete HID output report".into());
        }
        Ok(())
    }
    pub fn present(&self, key: u8, jpeg: &[u8]) -> Result<(), String> {
        packets::key_image(key, jpeg, |r| self.write(r))
    }
    pub fn present_strip(&self, jpeg: &[u8]) -> Result<(), String> {
        packets::strip_image(jpeg, |r| self.write(r))
    }
    pub fn heartbeat(&self) -> Result<(), String> {
        self.write(&packets::heartbeat())
    }
}
