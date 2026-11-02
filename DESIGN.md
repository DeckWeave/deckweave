# DeckWeave: desktop operating interface

The editor is an operating surface for configuring physical controls. Preserve the device geometry and readable previews; use the surrounding space for controls, settings and current state. The owner's direction is compact engineering software, with direct commands and predictable changes.

- One compact density. A 40 px application toolbar, 28 px form controls, approximately 29 px library rows and 12 px interface text. Do not reduce device preview pixels to make room for explanatory prose.
- Use the original DeckWeave artwork in `assets/branding` for application, window and tray identity. `scripts/generate-icon.mjs` copies it without redrawing it.
- Use the shared SVG vocabulary in `apps/desktop/src/Icon.svelte` and `icons.ts`. Keep text on unfamiliar commands; icon-only controls require an accessible name and a tooltip.
- Use neutral light/dark surfaces in `theme.css`, with a persisted accent (amber by default, matching the artwork). `density.css` owns spacing. Derive readable accent text/focus colors; theme native selects, options, checkboxes and window chrome as well as custom components. Hardware artwork retains its own colors.
- Put related values on one row or a compact grid. Two to five short choices belong in a visible button group. Long lists, application paths and arbitrary plugin choices can remain selects.
- Show errors, unknown hardware values and unverified input honestly. Move secondary explanations to tooltips or details; never hide a warning required to understand a pending hardware change.
- A device control click selects it. A library click selects an action and never changes an assignment. Assignment requires dropping it on the target or pressing the explicit Assign/Replace command, which names the target.
- Delete belongs to the focused device control or inspector. Typing or using the library must not delete a previously selected control's assignment.
- The library display editor reorders groups and actions by their handles. Group switches affect visibility in the library, preserving saved assignments. Execution enable/disable and package removal remain in application settings.
- Nested actions accept intentional drops from the same library. Sequences show a reorderable list beside the selected step's settings; Switch and gesture branches use direct tabs. Empty compositions are valid no-ops, without counter placeholders. Reject controller-incompatible drops and stale/cancelled transfers before changing configuration.
- Panel geometry uses the same resizable inspector as actions, with one selected region's pixel bounds and dial associations. Highlight that region on the device preview. Dropping an action opens its settings. Replacing the layout or removing a region requires an explicit inline confirmation; stale proposals cannot overwrite another page or a later edit.
- Settings save immediately. Hardware acceptance, software Test and merely selecting a field are distinct operations.

Resizable panes remain user controlled. Large geometric previews, unused space in a sparse settings page and third-party plugin inspector layouts should not be filled with decorative content.
