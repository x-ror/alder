import assert from "assert";
import fs from "fs";
import path from "path";

const dir = path.join(import.meta.dirname, "../snippets");
// An unescaped `$` that isn't a tabstop/variable silently breaks a snippet (e.g. Ruby's `$stdout`).
const strayDollar =
  /(?<!\\)\$(?![0-9{]|TM_|CLIPBOARD|CURRENT_|RANDOM|LINE_COMMENT|BLOCK_COMMENT|WORKSPACE_)/;
const registered = new Set(
  JSON.parse(
    fs.readFileSync(path.join(import.meta.dirname, "../package.json"), "utf8"),
  ).contributes.snippets.map(({ path: file }) => path.basename(file)),
);
for (const file of fs.readdirSync(dir).filter((f) => f.endsWith(".json"))) {
  assert(registered.has(file), `${file}: not in package.json snippets`);
  const seen = new Set();
  for (const [name, { prefix, body }] of Object.entries(
    JSON.parse(fs.readFileSync(path.join(dir, file), "utf8")),
  )) {
    assert(
      !seen.has(prefix),
      `${file}: duplicate prefix "${prefix}" (${name})`,
    );
    seen.add(prefix);
    const lines = typeof body === "string" ? [body] : body;
    assert(
      Array.isArray(lines) && lines.every((l) => typeof l === "string"),
      `${file}: bad body in ${name}`,
    );
    assert(
      !lines.some((l) => strayDollar.test(l)),
      `${file}: unescaped $ in ${name}`,
    );
  }
  console.log(`${file}: ${seen.size} snippets ok`);
}

// Zed picks a snippet file by the lowercased language name, and ours are symlinks into snippets/: both break silently.
const zedDir = path.join(import.meta.dirname, "../zed");
const zed = Bun.TOML.parse(
  fs.readFileSync(path.join(zedDir, "extension.toml"), "utf8"),
);
const zedLanguages = new Set(
  zed.language_servers["alder-ls"].languages.map((l) => l.toLowerCase()),
);
for (const file of zed.snippets) {
  assert(
    zedLanguages.has(path.basename(file, ".json")),
    `zed ${file}: no Zed language of that name`,
  );
  assert.equal(
    path.dirname(fs.realpathSync(path.join(zedDir, file))),
    fs.realpathSync(dir),
    `zed ${file}: not a link into snippets/`,
  );
}
console.log(`zed: ${zed.snippets.length} snippet files ok`);
