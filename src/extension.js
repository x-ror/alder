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
  "lamina.toggleErbTag": toggleErbTag,
  "lamina.toggleErbLineComment": toggleErbLineComment,
  "lamina.wrapInErb": wrapInErb,
  "lamina.convertToHaml": () => convert("haml"),
  "lamina.convertToSlim": () => convert("slim"),
  "lamina.convertToErb": () => convert("erb"),
  "lamina.extractPartial": extractPartial,
  "lamina.switchRelated": switchRelated,
  "lamina.toggleHamlPrefix": toggleActiveHamlLines,
  "lamina.toggleSlimTag": toggleSlimTag,
  "lamina.wrapInSlim": wrapInSlim,
  "lamina.togglePugLine": togglePugLine,
  "lamina.wrapInPug": wrapInPug,
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
