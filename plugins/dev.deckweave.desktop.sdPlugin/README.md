# Desktop Tools

Optional GPL-3.0-or-later starter actions: persistent note, host-owned countdown,
and notification sound. Requires DeckWeave host-services v1; unsupported hosts
report an error without opening another player or web server. Backend uses Node
22+ and the vendored SDK; it launches lazily for a visible action. Native note
and timer panels continue without its process.

Drag an action onto a key, open its settings, then press the key or use the
separate Test control. Repeated presses reopen the same saved panel. Inspector edits save immediately and update only changed fields of an existing
panel. They do not open a closed panel. Ordinary presses preserve changes made in
global Settings → Panels. Timer press starts/resumes; pause/reset are in
the panel and its global settings. Closing dismisses the view without cancelling
the countdown. Newly created timers use the selected completion sound; existing
legacy Timers assignments remain unchanged and silent.

Sound cues are generated in the host, not downloaded. No network or hardware
access. Image pinning and custom action callbacks are not implemented in this
slice. The inspector uses the repository's shared compact theme. All text is
escaped; plugin HTML never runs inside a desktop panel.
