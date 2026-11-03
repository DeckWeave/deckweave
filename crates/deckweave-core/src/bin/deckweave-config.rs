//! Offline configuration driver. It deliberately executes no effects or hardware I/O.
use deckweave_core::{Command, Core};
use serde_json::json;
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().ok_or("Usage: deckweave-config DATABASE")?;
    if args.next().is_some() {
        return Err("Usage: deckweave-config DATABASE".into());
    }
    let mut core = Core::open(std::path::Path::new(&path))?;
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{}", json!({"ok":true,"snapshot":core.snapshot()}))?;
    stdout.flush()?;
    let mut input = io::stdin().lock();
    loop {
        // A malformed or oversized request ends the offline session without mutation.
        let mut line = String::new();
        let bytes = std::io::Read::take(&mut input, 1_048_577).read_line(&mut line)?;
        if bytes == 0 {
            break;
        }
        if bytes > 1_048_576 || !line.ends_with('\n') {
            return Err("Configuration command exceeds the line limit".into());
        }
        let result = serde_json::from_str::<Command>(&line)
            .map_err(|error| error.to_string())
            .and_then(|command| {
                core.dispatch(command, "offline-setup")
                    .map_err(|e| e.to_string())
            });
        let reply = match result {
            Ok(snapshot) => json!({"ok":true,"snapshot":snapshot}),
            Err(error) => json!({"ok":false,"error":error}),
        };
        writeln!(stdout, "{reply}")?;
        stdout.flush()?;
    }
    Ok(())
}
