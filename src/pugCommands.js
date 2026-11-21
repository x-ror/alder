// Pug editing commands: cycle the line marker, wrap in output.
import * as vscode from "vscode";

import { INTERPOLATION_CLOSE, togglePugLines, wrapPug } from "./pug.js";
import { documentLines, selectedLines } from "./editor.js";

async function togglePugLine() {
  const ed = vscode.window.activeTextEditor;
  if (!ed) return;
  const edits = togglePugLines(
    documentLines(ed.document),
    new Set(ed.selections.flatMap(selectedLines)),
  );
  await ed.edit((eb) =>
    edits.forEach(([n, text]) => eb.replace(ed.document.lineAt(n).range, text)),
  );
}

async function wrapInPug() {
  const ed = vscode.window.activeTextEditor;
  if (!ed) return;
  const doc = ed.document;
  const lines = documentLines(doc);
  const wraps = ed.selections
    .filter((s) => s.isSingleLine)
    .map((s) => {
      const wrap = wrapPug(
        lines,
        s.start.line,
        s.start.character,
        s.end.character,
      );
      return wrap && { ...wrap, line: s.start.line };
    });
  if (wraps.length < ed.selections.length || wraps.some((w) => !w)) {
    vscode.window.showInformationMessage(
      "Wrap in Pug Output skips selections that span several lines or aren't text or a JavaScript expression Pug would take.",
    );
  }
  await ed.edit((eb) => {
    for (const { line, start, end, text } of wraps.filter(Boolean))
      eb.replace(new vscode.Range(line, start, line, end), text);
  });
  if (ed.selections.length === 1 && ed.selection.isEmpty && wraps[0]) {
    const p = ed.selection.active.translate(0, -INTERPOLATION_CLOSE.length);
    ed.selection = new vscode.Selection(p, p);
  }
}

export { togglePugLine, wrapInPug };
