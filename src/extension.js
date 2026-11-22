import * as vscode from "vscode";

import { registerServer, stopClient } from "./client.js";
import {
  toggleErbTag,
  toggleErbLineComment,
  wrapInErb,
} from "./erbCommands.js";
import { toggleActiveHamlLines } from "./hamlCommands.js";
import { toggleSlimTag, wrapInSlim } from "./slimCommands.js";
import { togglePugLine, wrapInPug } from "./pugCommands.js";
import { extractPartial } from "./extractPartial.js";
import { switchRelated } from "./relatedFile.js";
import { registerPreview } from "./preview.js";
import { convert } from "./convertTemplate.js";
import { registerRoutes } from "./railsRoutes.js";
import { registerErbAutoClose } from "./erbAutoClose.js";

const COMMANDS = {
  "alder.toggleErbTag": toggleErbTag,
  "alder.toggleErbLineComment": toggleErbLineComment,
  "alder.wrapInErb": wrapInErb,
  "alder.convertToHaml": () => convert("haml"),
  "alder.convertToSlim": () => convert("slim"),
  "alder.convertToErb": () => convert("erb"),
  "alder.extractPartial": extractPartial,
  "alder.switchRelated": switchRelated,
  "alder.toggleHamlPrefix": toggleActiveHamlLines,
  "alder.toggleSlimTag": toggleSlimTag,
  "alder.wrapInSlim": wrapInSlim,
  "alder.togglePugLine": togglePugLine,
  "alder.wrapInPug": wrapInPug,
};

function activate(context) {
  for (const [id, handler] of Object.entries(COMMANDS))
    context.subscriptions.push(vscode.commands.registerCommand(id, handler));
  registerServer(context);
  registerRoutes(context);
  registerErbAutoClose(context);
  registerPreview(context);
}

function deactivate() {
  return stopClient();
}

export { activate, deactivate };
