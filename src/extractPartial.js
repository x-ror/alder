// "Extract Selection to Partial": the selection moves into a new partial and a render call takes its place; in Pug, a new
// file next to the template and an `include`.
import * as vscode from "vscode";
import path from "path";

import { extractBlock, partialPath, renderPath } from "./partial.js";
import { pugExtractBlock, pugIncludePath, pugPartialPath } from "./pug.js";
import { engineOf } from "./languages.js";
import { confirmOverwrite } from "./workspace.js";
import { documentLines } from "./editor.js";

const PARTIAL_NAME_RE = /^([\w-]+\/)*[a-z_]\w*$/;
const INCLUDE_NAME_RE = /^([\w-]+\/)*[\w-]+$/;
const PUG = "jade";
const RENDER_CALL = {
  erb: (p) => `<%= render "${p}" %>`,
  haml: (p) => `= render "${p}"`,
  slim: (p) => `= render "${p}"`,
};

/** ERB, Haml and Slim: the block, the `_name` partial `render` finds and the call that replaces the block. */
async function railsPartial(doc, selection, engine) {
  // Haml and Slim nest by indentation, so only whole lines can move.
  const block = extractBlock(documentLines(doc), selection, engine !== "erb");
  if (!block) return null;
  const name = await vscode.window.showInputBox({
    prompt:
      "Partial name, without the leading _ and the extension (`card`, `shared/card`)",
    validateInput: (v) =>
      PARTIAL_NAME_RE.test(v)
        ? null
        : "Use a lowercase Ruby identifier, optionally prefixed by directories.",
  });
  if (!name) return null;
  const target = partialPath(doc.uri.fsPath, name);
  if (!target) {
    vscode.window.showErrorMessage(
      `"${name}" renders from app/views, but this file isn't under app/views.`,
    );
    return null;
  }
  const call = RENDER_CALL[engine](renderPath(doc.uri.fsPath, name));
  return { block, target, replacement: `${block.indent}${call}` };
}

/** Pug: the lines with what is nested under them, a file in the template's folder and the `include` that reads it. */
async function pugPartial(doc, selection) {
  const block = pugExtractBlock(documentLines(doc), selection);
  if (!block) return null;
  if (block.refused) {
    vscode.window.showInformationMessage(
      `Can't extract the selection: ${block.refused}`,
    );
    return null;
  }
  const name = await vscode.window.showInputBox({
    prompt:
      "File name, without the extension, from this template's folder (`row`, `users/row`)",
    validateInput: (v) =>
      INCLUDE_NAME_RE.test(v)
        ? null
        : "Use letters, digits, _ and -, optionally prefixed by folders.",
  });
  if (!name) return null;
  const target = pugPartialPath(doc.uri.fsPath, name);
  const include = pugIncludePath(doc.uri.fsPath, target);
  return { block, target, replacement: `${block.indent}include ${include}` };
}

async function extractPartial() {
  const ed = vscode.window.activeTextEditor;
  if (!ed || ed.selection.isEmpty) return;
  const doc = ed.document;
  const engine = engineOf(doc.languageId);
  if (engine !== PUG && !RENDER_CALL[engine]) return;
  if (doc.uri.scheme !== "file") {
    vscode.window.showInformationMessage(
      "Save the template before extracting a partial.",
    );
    return;
  }
  const extracted =
    engine === PUG
      ? await pugPartial(doc, ed.selection)
      : await railsPartial(doc, ed.selection, engine);
  if (!extracted) return;
  const { block, target, replacement } = extracted;
  if (path.resolve(target) === path.resolve(doc.uri.fsPath)) {
    vscode.window.showErrorMessage(
      `"${path.basename(target)}" is this template itself; pick another name.`,
    );
    return;
  }
  const targetUri = vscode.Uri.file(target);
  if (!(await confirmOverwrite(targetUri))) return;

  const eol = doc.eol === vscode.EndOfLine.CRLF ? "\r\n" : "\n";
  const { start, end } = block.range;
  // One edit, so a single undo restores the selection and a failed create leaves the template alone.
  const edit = new vscode.WorkspaceEdit();
  edit.createFile(targetUri, { overwrite: true });
  // A new document takes files.eol (CRLF on Windows) and would convert the inserted text to it.
  edit.set(targetUri, [
    vscode.TextEdit.setEndOfLine(doc.eol),
    vscode.TextEdit.insert(
      new vscode.Position(0, 0),
      block.lines.join(eol) + eol,
    ),
  ]);
  edit.replace(
    doc.uri,
    new vscode.Range(start.line, start.character, end.line, end.character),
    replacement,
  );
  if (!(await vscode.workspace.applyEdit(edit))) {
    vscode.window.showErrorMessage(`Couldn't create ${path.basename(target)}.`);
    return;
  }
  // Text edits leave the new partial open and unsaved.
  const partial = await vscode.workspace.openTextDocument(targetUri);
  await partial.save();
  await vscode.window.showTextDocument(partial, {
    viewColumn: vscode.ViewColumn.Beside,
    preview: true,
  });
}

export { extractPartial };
