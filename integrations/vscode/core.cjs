// Explicit local command IDs inspected in installed Typewriter 0.3.10 / Codex 26.1021.
// No request may provide a command ID, arguments, document text, paths or snippets.
'use strict';
const operations = Object.freeze({
  'report.preview': ['typstTypewriter.openPreview', 'local.typst-typewriter', true],
  'report.pdf': ['typstTypewriter.exportPdfQuick', 'local.typst-typewriter', true],
  'report.outline': ['outline.focus', null, true],
  'report.bold': ['typstPanel.bold', 'local.typst-typewriter', true],
  'report.italic': ['typstPanel.italic', 'local.typst-typewriter', true],
  'report.formula': ['typstPanel.insertFormula', 'local.typst-typewriter', true],
  'report.reference': ['typstPanel.insertVref', 'local.typst-typewriter', true],
  'report.image': ['typstTypewriter.insertImage', 'local.typst-typewriter', true],
  'report.table': ['typstTypewriter.editTable', 'local.typst-typewriter', true],
  'report.format': ['editor.action.formatDocument', 'myriad-dreamin.tinymist', true],
  'vscode.codexSidebar': ['chatgpt.openSidebar', 'openai.chatgpt', false],
  'vscode.codexNewChat': ['chatgpt.newChat', 'openai.chatgpt', false],
  'vscode.codexAddSelection': ['chatgpt.addToThread', 'openai.chatgpt', false],
});
function failure(code, message) {
  return Object.assign(new Error(message), { code });
}
function validateRequest(request, token) {
  if (
    !request ||
    typeof request !== 'object' ||
    Array.isArray(request) ||
    Object.keys(request).sort().join(',') !== 'id,operation,token' ||
    !Number.isSafeInteger(request.id) ||
    request.id < 1 ||
    request.id > 0xffff_ffff ||
    typeof request.token !== 'string' ||
    Buffer.byteLength(request.token) !== Buffer.byteLength(token)
  ) {
    throw failure('request', 'Invalid local workspace request');
  }
  const crypto = require('node:crypto');
  if (!crypto.timingSafeEqual(Buffer.from(request.token), Buffer.from(token))) {
    throw failure('auth', 'Workspace authentication failed; reload VS Code');
  }
  if (request.operation !== 'status' && !Object.hasOwn(operations, request.operation)) {
    throw failure('operation', 'Unsupported workspace operation; choose a listed command');
  }
}
function guard(vscode, operation) {
  if (!vscode.window.state.focused)
    throw failure(
      'not_focused',
      'Focus the intended VS Code window, then press the hardware command again',
    );
  if (!vscode.workspace.isTrusted)
    throw failure('untrusted', 'Trust this VS Code workspace before using document commands');
  const editor = vscode.window.activeTextEditor;
  if (
    operations[operation][2] &&
    (!editor ||
      editor.document.languageId !== 'typst' ||
      !['file', 'untitled'].includes(editor.document.uri.scheme))
  ) {
    throw failure('editor', 'Open and focus a Typst editor before using this report command');
  }
  if (operation === 'vscode.codexAddSelection' && (!editor || editor.selection.isEmpty)) {
    throw failure('selection', 'Select the intended editor text before adding it to Codex');
  }
  return editor;
}
function createDispatcher(vscode) {
  let busy = false;
  return async (operation, { signal } = {}) => {
    if (operation === 'status')
      return { focused: !!vscode.window.state.focused, operations: Object.keys(operations) };
    if (!Object.hasOwn(operations, operation))
      throw failure('operation', 'Unsupported workspace operation');
    if (busy)
      throw failure(
        'busy',
        'A workspace command is still running; finish its dialog before retrying',
      );
    if (signal?.aborted)
      throw failure('expired', 'Workspace request expired; press the command again');
    const original = guard(vscode, operation);
    const version = original?.document.version;
    const selection = original?.selection;
    busy = true;
    try {
      const [command, extensionId] = operations[operation];
      if (extensionId) {
        const extension = vscode.extensions.getExtension(extensionId);
        if (!extension)
          throw failure('extension', `Install or enable ${extensionId} in this VS Code window`);
        if (!extension.isActive) await extension.activate();
      }
      const available = await vscode.commands.getCommands(true);
      if (!available.includes(command))
        throw failure(
          'command',
          `Command ${command} is unavailable; enable its extension and reload VS Code`,
        );
      if (signal?.aborted)
        throw failure('expired', 'Workspace request expired; press the command again');
      const current = guard(vscode, operation);
      // Activation can yield. Never apply an old hardware press to a changed document/selection.
      if (
        current !== original ||
        current?.document.version !== version ||
        (selection && !selection.isEqual(current.selection))
      ) {
        throw failure('stale', 'The editor or selection changed; press the command again');
      }
      await vscode.commands.executeCommand(command);
      return { executed: operation };
    } finally {
      busy = false;
    }
  };
}
module.exports = { operations, validateRequest, createDispatcher, failure };
