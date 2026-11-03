'use strict';
// Runs inside VS Code's real extension host against a disposable .local document.
const vscode = require('vscode');
const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const crypto = require('node:crypto');
const net = require('node:net');
const { pathToFileURL } = require('node:url');
const { createDispatcher, operations } = require('./core.cjs');
const { startServer } = require('./server.cjs');
function request(pipe, token, id, operation) {
  return new Promise((resolve, reject) => {
    const socket = net.createConnection(pipe);
    let data = '';
    socket.setTimeout(10_000, () => {
      socket.destroy();
      reject(new Error('Fixture command timed out'));
    });
    socket.on('error', reject);
    socket.on('connect', () => socket.write(JSON.stringify({ id, token, operation }) + '\n'));
    socket.on('data', (chunk) => {
      data += chunk;
    });
    socket.on('end', () => {
      try {
        resolve(JSON.parse(data));
      } catch (error) {
        reject(error);
      }
    });
  });
}
exports.run = async () => {
  const resultPath = process.env.DECKWEAVE_FIXTURE_RESULT;
  const fixtureFile = process.env.DECKWEAVE_FIXTURE_DOCUMENT;
  if (!resultPath || !fixtureFile || !fixtureFile.includes(`${path.sep}.local${path.sep}`))
    throw new Error('Fixture paths must be explicit and isolated');
  const evidence = {
    realVsCodeHost: true,
    realTypewriter: false,
    commands: [],
    undoRestored: [],
    pipeAuthentication: false,
    document: fixtureFile,
  };
  let service, client;
  try {
    const typewriter = vscode.extensions.getExtension('local.typst-typewriter');
    const tinymist = vscode.extensions.getExtension('myriad-dreamin.tinymist');
    assert.ok(
      typewriter && tinymist,
      'Only installed fixture Typewriter/Tinymist packages are required',
    );
    await typewriter.activate();
    evidence.realTypewriter = true;
    evidence.typewriterVersion = typewriter.packageJSON.version;
    evidence.tinymistVersion = tinymist.packageJSON.version;
    const document = await vscode.workspace.openTextDocument(vscode.Uri.file(fixtureFile));
    await vscode.languages.setTextDocumentLanguage(document, 'typst');
    const editor = await vscode.window.showTextDocument(document);
    await vscode.commands.executeCommand('workbench.action.focusActiveEditorGroup');
    const commands = await vscode.commands.getCommands(true);
    for (const [operation, [command]] of Object.entries(operations).filter(([op]) =>
      op.startsWith('report.'),
    )) {
      assert.ok(commands.includes(command), `Registered installed command: ${command}`);
      evidence.commands.push({ operation, command, available: true });
    }
    const token = crypto.randomBytes(32).toString('hex');
    const pipe = `\\\\.\\pipe\\deckweave-real-fixture-${crypto.randomBytes(16).toString('hex')}`;
    service = await startServer({ pipe, token, dispatch: createDispatcher(vscode) });
    const denied = await request(pipe, '0'.repeat(64), 1, 'report.bold');
    assert.equal(denied.code, 'auth');
    evidence.pipeAuthentication = true;
    const { WorkspaceClient, NativeClient } = await import(
      pathToFileURL(
        path.resolve(__dirname, '../../plugins/dev.deckweave.workspace.sdPlugin/client.mjs'),
      ).href
    );
    client = new WorkspaceClient({
      native: new NativeClient({ locate: async () => process.env.DECKWEAVE_FIXTURE_HELPER }),
      discover: async () => [{ pipe, token }],
    });
    const baseline = document.getText();
    for (const operation of [
      'report.bold',
      'report.italic',
      'report.formula',
      'report.reference',
    ]) {
      await vscode.window.showTextDocument(document);
      const start = document.positionAt(baseline.indexOf('fixture'));
      const end = document.positionAt(baseline.indexOf('fixture') + 'fixture'.length);
      editor.selection = new vscode.Selection(start, end);
      const response = await client.request(operation);
      assert.equal(response.ok, true, response.error);
      evidence.nativeCodeFocusVerified = true;
      assert.notEqual(
        document.getText(),
        baseline,
        `${operation} changes only the disposable fixture`,
      );
      await vscode.commands.executeCommand('leaveSnippet');
      let undoSteps = 0;
      // Typewriter inserts required imports as a separate normal editor undo entry.
      while (document.getText() !== baseline && undoSteps < 8) {
        await vscode.commands.executeCommand('undo');
        undoSteps++;
      }
      assert.equal(document.getText(), baseline, `${operation} retains normal undo`);
      evidence.undoRestored.push({ operation, undoSteps });
    }
    assert.equal(document.getText(), baseline);
    evidence.personalDocumentTouched = false;
    evidence.codexAgentStarted = false;
    evidence.passed = true;
  } catch (error) {
    evidence.passed = false;
    evidence.error = String(error.stack || error);
    throw error;
  } finally {
    service?.close();
    client?.close();
    fs.writeFileSync(resultPath, JSON.stringify(evidence, null, 2) + '\n');
  }
};
