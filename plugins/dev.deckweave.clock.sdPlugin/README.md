# Date & Time

Digital time, date and weekday for keys and native Panel regions. Local operation,
no network, no subprocess and no cloud account. A shared one-second display tick
runs only while the plugin is active; unchanged images are not sent again.

Settings: `language` (`ru`/`en`), optional IANA `timezone` (empty means the computer's
zone), and `seconds` (default false). Unknown zones render an explicit error and
are rejected by the inspector. Settings save on change.

Run `npm test`, `npm run validate` and `npm run pack`. The standalone archive
includes the vendored SDK and licenses. Built for native 112×112 keys and variable
Panel regions; no downscaled analog clock text.
