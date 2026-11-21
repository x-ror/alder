// Pug oracle: runs templates through Pug 2.0.4's own lexer, comment stripper and parser in Node (what `pug.compile`
// does before linking) and records the first error, or an outline of the AST, for `server/src/pug/parser.rs` to match.
//   node test/oracle/pug.js           checks test/oracle/pug_cases.json against Pug (UPDATE=1 rewrites it), then
//                                     records Pug's own test templates and runs the Rust comparison on them
// PUG_PATH: a directory where `pug` 2.0.x resolves (`npm install pug@2.0.4` there). Without it the oracle is skipped.
// `bun test/oracle.js pug` imports the helpers below to compare Pug with `ruby-templates check`.
import { spawnSync } from "child_process";
import fs from "fs";
import { createRequire } from "module";
import os from "os";
import path from "path";
import { fileURLToPath } from "url";

export const VERSION = "2.0.4";
export const casesFile = path.join(import.meta.dirname, "pug_cases.json");
const manifest = path.join(
  import.meta.dirname,
  "..",
  "..",
  "server",
  "Cargo.toml",
);
const cacheDir = path.resolve(
  process.env.ORACLE_CACHE ?? path.join(os.tmpdir(), "ruby-templates-oracle"),
);
// Pug's own templates at the tag, relative to the checkout: compiled cases, cases that must fail, lexer cases.
const upstreamDirs = [
  "packages/pug/test/cases",
  "packages/pug/test/anti-cases",
  "packages/pug-lexer/test/cases",
  "packages/pug-lexer/test/errors",
];

export const loadPug = () => {
  if (!process.env.PUG_PATH) return undefined;
  const fromPath = createRequire(
    path.join(path.resolve(process.env.PUG_PATH), "noop.js"),
  );
  let pugJson;
  try {
    pugJson = fromPath.resolve("pug/package.json");
  } catch {
    return undefined;
  }
  const { version } = JSON.parse(fs.readFileSync(pugJson, "utf8"));
  if (!version.startsWith("2.0.")) return undefined;
  // The lexer and parser Pug itself loads, at the versions it pins.
  const fromPug = createRequire(pugJson);
  // pug-parser warns about style (several attribute lists, `.jade`), which isn't an error.
  console.warn = () => {};
  return {
    version,
    lex: fromPug("pug-lexer"),
    strip: fromPug("pug-strip-comments"),
    parse: fromPug("pug-parser"),
  };
};

const attr = (a) =>
  `${a.name}${a.mustEscape ? "=" : "!="}${a.val === true ? "true" : a.val}@${a.line}:${a.column}`;
const attrList = (node) =>
  [
    ...(node.attrs ?? []).map(attr),
    ...(node.attributeBlocks ?? []).map(
      (b) => `&(${b.val})@${b.line}:${b.column}`,
    ),
  ].join(" ");

// One line per node, depth first; blocks only group. `outline` in parser.rs writes the same.
const outline = (node, out = []) => {
  if (!node) return out;
  const at = `@${node.line}:${node.column}`;
  const walk = (child) => outline(child, out);
  switch (node.type) {
    case "Block":
      node.nodes.forEach(walk);
      break;
    case "NamedBlock":
      out.push(`block ${node.mode} ${node.name}${at}`);
      node.nodes.forEach(walk);
      break;
    case "Tag":
    case "InterpolatedTag":
      out.push(
        `${node.type === "Tag" ? node.name : `#{${node.expr}}`}${node.selfClosing ? "/" : ""}${node.textOnly ? "." : ""} (${attrList(node)})${at}`,
      );
      walk(node.block);
      break;
    case "Mixin":
      out.push(
        node.call
          ? `+${node.name}(${node.args ?? ""}) (${attrList(node)})${at}`
          : `mixin ${node.name}(${node.args ?? ""})${at}`,
      );
      walk(node.block);
      break;
    case "Text":
      out.push(
        `text${node.isHtml ? " html" : ""} ${JSON.stringify(node.val)}${at}`,
      );
      break;
    case "Code":
      out.push(
        `code${node.buffer ? (node.mustEscape ? "=" : "!=") : "-"}${node.isInline ? " inline" : ""} ${JSON.stringify(node.val)}${at}`,
      );
      walk(node.block);
      break;
    case "Conditional":
      out.push(`if ${node.test}${at}`);
      walk(node.consequent);
      if (node.alternate?.type === "Block")
        out.push(`else@${node.alternate.line}`);
      walk(node.alternate);
      break;
    case "While":
      out.push(`while ${node.test}${at}`);
      walk(node.block);
      break;
    case "Each":
      out.push(`each ${node.val},${node.key ?? ""} in ${node.obj}${at}`);
      walk(node.block);
      if (node.alternate) out.push(`else@${node.alternate.line}`);
      walk(node.alternate);
      break;
    case "Case":
      out.push(`case ${node.expr}${at}`);
      walk(node.block);
      break;
    case "When":
      out.push(`when ${node.expr}${at}`);
      walk(node.block);
      break;
    case "MixinBlock":
      out.push(`mixin-block${at}`);
      break;
    case "YieldBlock":
      out.push(`yield${at}`);
      break;
    case "Extends":
      out.push(
        `extends ${node.file.path}@${node.file.line}:${node.file.column}${at}`,
      );
      break;
    case "Include":
      out.push(
        `include ${node.file.path}@${node.file.line}:${node.file.column}${at}`,
      );
      walk(node.block);
      break;
    case "RawInclude":
      out.push(
        `include${node.filters.map((f) => `:${f.name}(${attrList(f)})@${f.line}:${f.column}`).join("")} ${node.file.path}@${node.file.line}:${node.file.column}${at}`,
      );
      break;
    case "Filter":
      out.push(`:${node.name}(${attrList(node)})${at}`);
      walk(node.block);
      break;
    case "Comment":
    case "BlockComment":
      out.push(`//${node.buffer ? "" : "-"} ${JSON.stringify(node.val)}${at}`);
      walk(node.block);
      break;
    case "Doctype":
      out.push(`doctype ${node.val}${at}`);
      break;
    default:
      throw new Error(`unknown node type ${node.type}`);
  }
  return out;
};

// Pug's first error: its code, message, line and column; an error Pug doesn't word itself (a JavaScript TypeError,
// character-parser's or assert's) has no position. Node's wording of a TypeError is recorded as it is.
const official = (pug, source) => {
  try {
    return {
      error: null,
      outline: outline(pug.parse(pug.strip(pug.lex(source)))),
    };
  } catch (e) {
    if (e.code?.startsWith("PUG:")) {
      return {
        error: { code: e.code, message: e.msg, line: e.line, column: e.column },
      };
    }
    return {
      error: {
        code: e.code ?? e.name,
        message: e.message,
        line: null,
        column: null,
      },
    };
  }
};

export const record = (pug, source) => ({ source, ...official(pug, source) });

// The recorded answers must still be Pug's: the Rust tests trust them.
const checkRecorded = (pug) => {
  const cases = JSON.parse(fs.readFileSync(casesFile, "utf8"));
  const fresh = cases.map(({ source }) => record(pug, source));
  if (process.env.UPDATE) {
    fs.writeFileSync(casesFile, `${JSON.stringify(fresh, null, 2)}\n`);
    console.log(
      `pug_cases.json: ${fresh.length} cases recorded; run \`bun fmt test/oracle/pug_cases.json\``,
    );
    return true;
  }
  const stale = cases.filter(
    (c, i) => JSON.stringify(c) !== JSON.stringify(fresh[i]),
  );
  stale.forEach((c) => console.log(`stale: ${JSON.stringify(c.source)}`));
  console.log(
    `pug_cases.json: ${cases.length - stale.length}/${cases.length} as Pug answers`,
  );
  return stale.length === 0;
};

const checkout = () => {
  const dir = path.join(cacheDir, `pug-${VERSION}`);
  if (fs.existsSync(dir)) return dir;
  fs.mkdirSync(cacheDir, { recursive: true });
  const partial = `${dir}.partial`;
  fs.rmSync(partial, { recursive: true, force: true });
  const clone = spawnSync(
    "git",
    [
      "clone",
      "--depth",
      "1",
      "--branch",
      `pug@${VERSION}`,
      "https://github.com/pugjs/pug.git",
      partial,
    ],
    { encoding: "utf8" },
  );
  if (clone.status !== 0) throw new Error(`git clone failed: ${clone.stderr}`);
  fs.renameSync(partial, dir);
  return dir;
};

// Pug's test templates, each with Pug's answer for it.
export const upstreamTemplates = (pug) => {
  const root = checkout();
  return upstreamDirs.flatMap((dir) =>
    fs
      .readdirSync(path.join(root, dir))
      .filter((name) => name.endsWith(".pug"))
      .sort()
      .map((name) => ({
        label: `${dir}/${name}`,
        ...record(pug, fs.readFileSync(path.join(root, dir, name), "utf8")),
      })),
  );
};

// Pug's test templates recorded into the cache, then compared by the ignored Rust test that reads them.
const checkUpstream = (pug) => {
  const results = upstreamTemplates(pug);
  const file = path.join(cacheDir, `pug-${VERSION}-results.json`);
  fs.writeFileSync(file, JSON.stringify(results));
  console.log(`upstream: ${results.length} templates from pug@${VERSION}`);
  const test = spawnSync(
    "cargo",
    [
      "test",
      "--manifest-path",
      manifest,
      "--lib",
      "pug::parser::tests::upstream_templates",
      "--",
      "--ignored",
      "--nocapture",
    ],
    {
      encoding: "utf8",
      env: { ...process.env, PUG_ORACLE_RESULTS: file },
      stdio: ["ignore", "inherit", "inherit"],
    },
  );
  return test.status === 0;
};

export const INSTALL_HINT = `set PUG_PATH to a directory where pug ${VERSION} is installed (npm install pug@${VERSION})`;

if (path.resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) {
  const pug = loadPug();
  if (!pug) {
    console.log(`pug oracle skipped: ${INSTALL_HINT}`);
    process.exit(0);
  }
  console.log(`pug ${pug.version}`);
  const recordedAgree = checkRecorded(pug);
  const upstreamAgree = process.env.UPDATE || checkUpstream(pug);
  process.exit(recordedAgree && upstreamAgree ? 0 : 1);
}
