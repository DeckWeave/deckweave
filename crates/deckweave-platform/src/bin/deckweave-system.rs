//! Optional plugin helper. One bounded JSON request and one JSON response per line on stdio.
//! No subprocesses, network access, or audio changes without an explicit validated command.
#![cfg_attr(windows, windows_subsystem = "windows")]
use deckweave_platform::audio::{AudioCommand, AudioSession};
use deckweave_platform::gpu::{GpuSample, GpuSampler};
use serde_json::{Value, json};
use std::io::{self, BufRead, Write};

fn read_request(reader: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut line = Vec::new();
    loop {
        let buffer = reader.fill_buf()?;
        if buffer.is_empty() {
            return Ok((!line.is_empty()).then_some(line));
        }
        let count = buffer
            .iter()
            .position(|b| *b == b'\n')
            .map_or(buffer.len(), |p| p + 1);
        if line.len() + count > 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Request exceeds 1024 bytes",
            ));
        }
        line.extend_from_slice(&buffer[..count]);
        reader.consume(count);
        if line.last() == Some(&b'\n') {
            return Ok(Some(line));
        }
    }
}

fn handle(line: &[u8], session: &mut Option<AudioSession>, gpu: &mut Option<GpuSampler>) -> Value {
    let result = (|| -> Result<(u32, Value), String> {
        let mut value: Value = serde_json::from_slice(line).map_err(|e| e.to_string())?;
        let object = value.as_object_mut().ok_or("Expected JSON object")?;
        let id = object
            .remove("id")
            .and_then(|v| v.as_u64())
            .and_then(|n| u32::try_from(n).ok())
            .ok_or("Expected u32 request id")?;
        Ok((id, value))
    })();
    let (id, command) = match result {
        Ok(request) => request,
        Err(error) => return json!({"id": null, "ok": false, "error": error}),
    };
    if command.get("command").and_then(Value::as_str) == Some("gpu_sample") {
        if command.as_object().is_none_or(|object| object.len() != 1) {
            return json!({"id":id,"ok":false,"error":"gpu_sample takes no parameters"});
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(u64::MAX as u128) as u64;
        if gpu.is_none() {
            match GpuSampler::new() {
                Ok(sampler) => *gpu = Some(sampler),
                Err(reason) => {
                    return json!({"id":id,"ok":true,"state":GpuSample::unavailable(reason, now)});
                }
            }
        }
        return json!({"id":id,"ok":true,"state":gpu.as_mut().expect("sampler initialized").sample(now)});
    }
    let command = match serde_json::from_value::<AudioCommand>(command)
        .map_err(|e| e.to_string())
        .and_then(AudioCommand::validate)
    {
        Ok(command) => command,
        Err(error) => return json!({"id":null,"ok":false,"error":error}),
    };
    let result = (|| {
        if session.is_none() {
            *session = Some(AudioSession::new()?);
        }
        session
            .as_ref()
            .expect("session initialized")
            .execute(command)
    })();
    match result {
        Ok(state) => json!({"id": id, "ok": true, "state": state}),
        Err(error) => json!({"id": id, "ok": false, "error": error}),
    }
}

fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let mut session = None;
    let mut gpu = None;
    while let Some(line) = read_request(&mut input)? {
        serde_json::to_writer(&mut output, &handle(&line, &mut session, &mut gpu))?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn protocol_bounds_and_invalid_commands_never_reach_audio() {
        assert!(read_request(&mut &vec![b'x'; 1025][..]).is_err());
        assert_eq!(
            read_request(&mut &b"{}\n"[..]).unwrap(),
            Some(b"{}\n".to_vec())
        );
        assert!(
            !handle(
                br#"{"id":1,"command":"set_volume","value":999}"#,
                &mut None,
                &mut None
            )["ok"]
                .as_bool()
                .unwrap()
        );
        assert!(
            !handle(
                br#"{"id":2,"command":"launch","path":"x"}"#,
                &mut None,
                &mut None
            )["ok"]
                .as_bool()
                .unwrap()
        );
        assert!(
            !handle(
                br#"{"id":3,"command":"gpu_sample","extra":true}"#,
                &mut None,
                &mut None
            )["ok"]
                .as_bool()
                .unwrap()
        );
    }
}
