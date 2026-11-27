// The Rust language server: diagnostics, go-to-partial, completions, hover and formatting.
import * as vscode from "vscode";
import path from "path";
import fs from "fs";
import {
  LanguageClient,
  State,
  TransportKind,
} from "vscode-languageclient/node";

import { SECTION, settings } from "./workspace.js";

/** @type {LanguageClient | undefined} */
let client;
/** @type {vscode.LogOutputChannel | undefined} */
let output;
// A restarted server has lost what was sent to the previous one.
const startListeners = new Set();
// stop() hands concurrent callers one shared promise, so overlapping restarts would each start a server.
let queue = Promise.resolve();

const log = (msg) => output?.appendLine(`[alder] ${msg}`);

function serverBinaryName() {
  return process.platform === "win32" ? "alder-ls.exe" : "alder-ls";
}

/** Locate the language server: setting → bundled per-platform binary → dev build → PATH. */
function findServer(context) {
  const configured = settings().get("server.path", "");
  if (configured && fs.existsSync(configured)) return configured;
  const target = `${process.platform}-${process.arch}`;
  const candidates = [
    context.asAbsolutePath(path.join("bin", target, serverBinaryName())),
    context.asAbsolutePath(
      path.join("server", "target", "release", serverBinaryName()),
    ),
    context.asAbsolutePath(
      path.join("server", "target", "debug", serverBinaryName()),
    ),
  ];
  for (const c of candidates) if (fs.existsSync(c)) return c;
  return serverBinaryName(); // hope it is on PATH
}

/** The server's per-engine settings, read once at startup. */
function initializationOptions(config) {
  const switches = (lang, format = {}) => ({
    diagnostics: { enabled: config.get(`${lang}.diagnostics.enabled`, true) },
    format: { enabled: config.get(`${lang}.format.enabled`, true), ...format },
  });
  return {
    // vscode-languageclient honours the server's dynamic registrations that sync the files its quick fixes edit.
    syncsFixTargets: true,
    slim: {
      extraEngines: config.get("slim.extraEngines", []),
      disabledDiagnostics: config.get("slim.disabledDiagnostics", []),
      ...switches("slim"),
    },
    erb: {
      disabledDiagnostics: config.get("erb.disabledDiagnostics", []),
      ...switches("erb", {
        alignComments: config.get("erb.format.alignComments", false),
      }),
    },
    haml: {
      disabledDiagnostics: config.get("haml.disabledDiagnostics", []),
      ...switches("haml"),
    },
    jbuilder: {
      disabledDiagnostics: config.get("jbuilder.disabledDiagnostics", []),
    },
    liquid: {
      disabledDiagnostics: config.get("liquid.disabledDiagnostics", []),
    },
    pug: {
      disabledDiagnostics: config.get("pug.disabledDiagnostics", []),
      ...switches("pug"),
    },
  };
}

// The server reads these at startup, so a change restarts it.
const SERVER_SETTINGS = [
  "server",
  "slim.extraEngines",
  "slim.disabledDiagnostics",
  "erb.disabledDiagnostics",
  "haml.disabledDiagnostics",
  "jbuilder.disabledDiagnostics",
  "liquid.disabledDiagnostics",
  "pug.disabledDiagnostics",
  "erb.format.alignComments",
  ...["erb", "haml", "slim", "pug"].flatMap((lang) => [
    `${lang}.diagnostics.enabled`,
    `${lang}.format.enabled`,
  ]),
].map((key) => `${SECTION}.${key}`);

// Not awaited by callers: the restart queue must not wait for the notification to be dismissed.
async function offerHelp(msg) {
  const pick = await vscode.window.showWarningMessage(
    msg,
    "Open settings",
    "Show log",
  );
  if (pick === "Open settings")
    await vscode.commands.executeCommand(
      "workbench.action.openSettings",
      `${SECTION}.server`,
    );
  if (pick === "Show log") output.show();
}

async function disposeQuietly(languageClient) {
  try {
    await languageClient.dispose();
  } catch (e) {
    log(`stopping language server: ${e}`);
  }
}

async function doStart(context) {
  const config = settings();
  if (!config.get("server.enabled", true)) return;
  const command = findServer(context);
  log(`starting language server: ${command}`);

  const started = new LanguageClient(
    SECTION,
    "Alder Language Server",
    { command, transport: TransportKind.stdio },
    {
      // Scheme-less selectors also match git diff sides and chat code blocks, duplicating diagnostics.
      // `ruby` is contributed only for Jbuilder/Builder views; other Ruby files are not templates.
      // `jade` is VS Code's own Pug language, which we don't contribute.
      documentSelector: [
        ...context.extension.packageJSON.contributes.languages
          .map(({ id }) => id)
          .filter((id) => id !== "ruby")
          .concat("jade")
          .flatMap((id) => [
            { scheme: "file", language: id },
            { scheme: "untitled", language: id },
          ]),
        {
          scheme: "file",
          language: "ruby",
          pattern: "**/*.{jbuilder,builder}",
        },
        // Controllers only for go to definition / links into their views; the server ignores them otherwise.
        {
          scheme: "file",
          language: "ruby",
          pattern: "**/app/controllers/**/*_controller.rb",
        },
      ],
      outputChannel: output,
      // Our catch below reports the failure; returning false stops the client from also showing its own.
      initializationFailedHandler: () => false,
      // The server registers its own file watchers, and syncs the locale files and Stimulus controllers its fixes edit.
      initializationOptions: initializationOptions(config),
    },
  );
  client = started;
  // Also after the client restarts a crashed server by itself, which doesn't go through doStart.
  started.onDidChangeState(({ newState }) => {
    if (newState === State.Running)
      for (const listener of startListeners) listener();
  });
  try {
    await started.start();
  } catch (e) {
    client = undefined;
    await disposeQuietly(started);
    const msg =
      `Alder language server failed to start (${command}). ` +
      "Install the package built for your platform, build the server from the repository with `bun run build:server`, or set alder.server.path.";
    log(`${msg}\n${e?.message || e}`);
    offerHelp(msg).catch((err) => log(`opening help: ${err?.message ?? err}`));
  }
}

async function doStop() {
  const old = client;
  client = undefined;
  if (old) await disposeQuietly(old);
}

const serialize = (task) =>
  (queue = queue.then(task).catch((e) => log(e?.message ?? e)));

const startClient = (context) => serialize(() => doStart(context));
const stopClient = () => serialize(doStop);
const restartClient = (context) =>
  serialize(async () => {
    await doStop();
    await doStart(context);
  });

const onServerStarted = (listener) => {
  startListeners.add(listener);
  return { dispose: () => startListeners.delete(listener) };
};

/** Send a notification to the running server; dropped while it is stopped (start listeners resend state). */
const notifyServer = (method, params) =>
  client?.isRunning() ? client.sendNotification(method, params) : undefined;

/** Starts the server, restarting it on the restart command or a change to a setting it reads; returns its log. */
function registerServer(context) {
  output = vscode.window.createOutputChannel("Alder", { log: true });
  context.subscriptions.push(
    output,
    vscode.commands.registerCommand("alder.restartServer", () =>
      restartClient(context),
    ),
    vscode.workspace.onDidChangeConfiguration((e) => {
      if (SERVER_SETTINGS.some((s) => e.affectsConfiguration(s)))
        void restartClient(context);
    }),
  );
  void startClient(context);
}

export { registerServer, log, stopClient, onServerStarted, notifyServer };
