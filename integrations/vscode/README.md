# DeckWeave local VS Code companion

Source-shipped extension for the personal hardware workspace. It accepts a fixed
operation name on a random Windows named pipe, authenticated by a random 256-bit
token. Discovery files live in `%LOCALAPPDATA%\DeckWeave\workspace-vscode`; the
directory DACL grants the current Windows SID full control and removes inherited
access. Nothing listens on TCP or HTTP. Tokens and document content are never
returned to the hardware plugin.

Run the isolated checks first, from the repository root:

```powershell
node --test integrations/vscode/companion.test.cjs plugins/dev.deckweave.workspace.sdPlugin/workspace.test.mjs
cargo test -p deckweave-workspace --locked
```

Install only after reviewing the fixture results and source:

```powershell
powershell -NoProfile -File integrations/vscode/install.ps1
```

Reload the intended local VS Code window. The installer copies these source
files into a versioned extension directory, backs up a previous matching copy,
and does not edit keybindings or user/workspace settings. It does not enable the
extension in remote extension hosts or untrusted workspaces. Remove the
`deckweave.workspace-0.1.0` extension directory to uninstall, then reload VS Code.

The hardware client focuses VS Code, asks local endpoints only for their focus
state, and executes against exactly one focused window. The companion checks
focus again immediately before invoking a command. Every report operation
requires an active Typst editor. If extension activation changes the editor,
document version, selection or focus, the command fails without an edit.
Operations invoke the installed extension's normal commands; document edits
retain their normal VS Code undo behavior. Image/table commands can open normal
extension dialogs. Finish a dialog before pressing another command. A timed-out
or disconnected request is not retried automatically.

Installed command mapping verified for Typewriter 0.3.10 and Codex 26.1021.22027:

| Operation                  | VS Code command                                         |
| -------------------------- | ------------------------------------------------------- |
| report.preview             | typstTypewriter.openPreview                             |
| report.pdf                 | typstTypewriter.exportPdfQuick                          |
| report.outline             | outline.focus                                           |
| report.bold / italic       | typstPanel.bold / typstPanel.italic                     |
| report.formula / reference | typstPanel.insertFormula / typstPanel.insertVref        |
| report.image / table       | typstTypewriter.insertImage / typstTypewriter.editTable |
| report.format              | editor.action.formatDocument (Tinymist enabled)         |
| vscode.codexSidebar        | chatgpt.openSidebar                                     |
| vscode.codexNewChat        | chatgpt.newChat                                         |
| vscode.codexAddSelection   | chatgpt.addToThread                                     |

`vscode.codexAddSelection` requires a nonempty selection and adds it to the Codex
composer only after that explicit hardware operation. No operation sends a
prompt, starts an agent task, approves a command, archives a chat, or publishes
content. No command accepts caller arguments, editor text, file paths or arbitrary
VS Code command IDs. Errors appear on the hardware and in the inspector; command
errors also appear as an actionable VS Code notification.

The small unit fixture checks dispatch guards and actual bounded named-pipe
framing with a fake editor. `node integrations/vscode/run-native-fixture.mjs`
also launches a separate real VS Code extension host with isolated portable
data and copies of only the installed Typewriter/Tinymist packages. It verifies
all ten report command registrations, rejects a wrong pipe token, verifies Code
foreground through the real native helper and WorkspaceClient, executes
bold/italic/formula/reference on a disposable `.local` Typst file, and restores
the original text with normal undo. It does not install the companion or open
personal documents. This machine's sandbox could not start the renderer; the
successful run used elevated sandbox access for that isolated process.

[Recorded evidence](evidence/2026-11-03-fixture.json): Typewriter 0.3.10,
Tinymist 0.15.10, four successful real edits and complete undo restoration.
Bold/italic use one undo step. Formula/reference require two because Typewriter
inserts a required import as a separate normal editor edit. Preview, PDF,
image/table dialogs and formatting were verified as registered commands;
their UI/output and the Codex extension were not automatically executed.
Production companion activation/credential-directory ACL and physical hardware
presses remain separate installation checks.
