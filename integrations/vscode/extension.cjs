'use strict';
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');
const crypto = require('node:crypto');
const { execFileSync } = require('node:child_process');
const { createDispatcher } = require('./core.cjs');
const { startServer } = require('./server.cjs');
async function activate(context) {
  if (process.platform !== 'win32') return;
  try {
    const directory = path.join(process.env.LOCALAPPDATA, 'DeckWeave', 'workspace-vscode');
    // Use the process SID rather than environment-supplied account names for the directory DACL.
    execFileSync(
      path.join(process.env.WINDIR, 'System32', 'WindowsPowerShell', 'v1.0', 'powershell.exe'),
      [
        '-NoLogo',
        '-NoProfile',
        '-NonInteractive',
        '-File',
        path.join(__dirname, 'secure-directory.ps1'),
        '-Directory',
        directory,
      ],
      { windowsHide: true, timeout: 5000, stdio: 'pipe' },
    );
    const nonce = crypto.randomBytes(20).toString('hex');
    const token = crypto.randomBytes(32).toString('hex');
    const pipe = `\\\\.\\pipe\\deckweave-vscode-${nonce}`;
    const endpoint = path.join(directory, `${nonce}.json`);
    const service = await startServer({
      pipe,
      token,
      dispatch: createDispatcher(vscode),
      onError: (message) => vscode.window.showErrorMessage(`DeckWeave: ${message}`),
    });
    try {
      fs.writeFileSync(
        endpoint,
        JSON.stringify({ version: 1, pipe, token, pid: process.pid, createdAt: Date.now() }),
        { flag: 'wx', mode: 0o600 },
      );
    } catch (error) {
      service.close();
      throw error;
    }
    context.subscriptions.push({
      dispose() {
        service.close();
        try {
          fs.unlinkSync(endpoint);
        } catch {}
      },
    });
  } catch (error) {
    vscode.window.showErrorMessage(
      `DeckWeave companion could not start: ${error.message}. Reinstall the local companion and reload VS Code.`,
    );
  }
}
exports.activate = activate;
