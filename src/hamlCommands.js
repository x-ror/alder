// Haml editing commands: cycle the Ruby prefix.
import * as vscode from "vscode";

import { toggleHamlLines } from "./haml.js";
import { selectedLines, documentLines } from "./editor.js";

async function toggleActiveHamlLines() {
  const ed = vscode.window.activeTextEditor;
  if (!ed) return;
  const edits = toggleHamlLines(
    documentLines(ed.document),
    new Set(ed.selections.flatMap(selectedLines)),
  );
  await ed.edit((eb) =>
    edits.forEach(([n, text]) => eb.replace(ed.document.lineAt(n).range, text)),
  );
}

export { toggleActiveHamlLines };
