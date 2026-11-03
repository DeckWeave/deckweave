//! A private child-process stdio channel; never listens on a network socket.
#![cfg_attr(windows, windows_subsystem = "windows")]
use std::io::{self, BufRead, Write};

fn frame(input: &mut impl BufRead) -> io::Result<Option<Vec<u8>>> {
    let mut result = vec![];
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok((!result.is_empty()).then_some(result));
        }
        let count = available
            .iter()
            .position(|b| *b == b'\n')
            .map_or(available.len(), |i| i + 1);
        if result.len() + count > 1024 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Workspace frame exceeds 1024 bytes",
            ));
        }
        result.extend_from_slice(&available[..count]);
        input.consume(count);
        if result.last() == Some(&b'\n') {
            return Ok(Some(result));
        }
    }
}
fn main() -> io::Result<()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let stdout = io::stdout();
    let mut output = stdout.lock();
    while let Some(line) = frame(&mut input)? {
        serde_json::to_writer(&mut output, &deckweave_workspace::handle(&line))?;
        output.write_all(b"\n")?;
        output.flush()?;
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frames_are_bounded_before_deserialization() {
        assert!(frame(&mut &vec![b'x'; 1025][..]).is_err());
        assert_eq!(frame(&mut &b"{}\n"[..]).unwrap().unwrap(), b"{}\n");
    }
}
