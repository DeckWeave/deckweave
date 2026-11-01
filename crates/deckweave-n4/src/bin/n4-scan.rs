fn main() {
    #[cfg(windows)]
    match deckweave_n4::discover() {
        Ok(devices) => println!("{}", serde_json::to_string_pretty(&devices).unwrap()),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1)
        }
    }
}
