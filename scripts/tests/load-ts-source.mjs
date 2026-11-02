import { readFile } from 'node:fs/promises';
import ts from 'typescript';

// Keep tests runnable on the documented Node 22.12 minimum, before native type stripping.
export async function loadTypeScript(url) {
  const source = await readFile(url, 'utf8');
  const { outputText } = ts.transpileModule(source, {
    compilerOptions: { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.ES2022 },
    fileName: url.pathname,
  });
  return import('data:text/javascript;base64,' + Buffer.from(outputText).toString('base64'));
}
