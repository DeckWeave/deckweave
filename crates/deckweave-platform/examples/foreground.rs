fn main() {
    println!("session_locked={:?}", deckweave_platform::session_locked());
    println!("{}", deckweave_platform::foreground().unwrap_or_default());
}
