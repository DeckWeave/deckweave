# Host services model

Validated panel records, independent SQLite store and native countdown runs.
No Tauri, hardware, plugin process or audio dependencies. See
[implementation contract](../../docs/designs/host-feedback/design_draft.md).

Run `cargo test -p deckweave-services --locked`. Desktop rendering, authentication
and bounded queues live in `apps/desktop/src-tauri/src/host_services.rs`.
