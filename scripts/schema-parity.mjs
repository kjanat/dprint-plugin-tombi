import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const plugin = resolve(process.argv[2] ?? "target/wasm32-unknown-unknown/wasm-release/dprint_plugin_tombi.wasm");
const directory = mkdtempSync(join(tmpdir(), "tombi-schema-parity-"));
const tombi = process.env.TOMBI_BIN ?? "tombi";
const kprint = process.env.KPRINT_BIN ?? "kprint";
const fixtures = fileURLToPath(new URL("../tests/fixtures/actionlint/", import.meta.url));
const source = `[files]
excludes = []

[local-actions]
example = 'action'

[[ignores]]
rule = 'missing-local-action'

[settings]
z = 1
a = 2
`;
const schema = {
  type: "object",
  properties: {
    files: { type: "object" },
    ignores: { type: "array", items: { type: "object" } },
    "local-actions": { type: "object" },
    settings: { $ref: "parts/settings.json" },
  },
  "x-tombi-table-keys-order": { properties: "schema" },
};

function run(binary, args, input) {
  const result = spawnSync(binary, args, {
    cwd: directory,
    input,
    encoding: "utf8",
    timeout: 300_000,
    env: { ...process.env, NO_COLOR: "1", DPRINT_CACHE_DIR: join(directory, ".cache") },
  });
  if (result.error) {
    result.error.message += `\n${result.stdout}\n${result.stderr}`;
    throw result.error;
  }
  assert.equal(result.status, 0, `${binary} ${args.join(" ")}\n${result.stdout}\n${result.stderr}`);
  return result.stdout;
}

function compare(label, filename, input = source) {
  const relativeFilename = filename;
  filename = join(directory, filename);
  writeFileSync(filename, input);
  const expected = run(tombi, ["format", "--offline", "--stdin-filename", filename, "-"], input);
  const actual = run(kprint, ["fmt", "--stdin", filename], input);
  assert.equal(actual, expected, `${label}: kprint and tombi format disagree`);
  assert.equal(
    run(kprint, ["fmt", "--stdin", relativeFilename], input),
    actual,
    `${label}: relative filename changed output`,
  );
  run(kprint, ["fmt", filename]);
  assert.equal(readFileSync(filename, "utf8"), expected, `${label}: formatting the file changed output`);
  assert.equal(run(kprint, ["fmt", "--stdin", filename], actual), actual, `${label}: second pass changed output`);
  console.log(`Passed: ${label}`);
  return actual;
}

try {
  assert.match(run(tombi, ["--version"]), /\b1\.11\.1\b/, "Use Tombi 1.11.1 for this comparison");
  mkdirSync(join(directory, "parts"));
  writeFileSync(join(directory, "schema.json"), JSON.stringify(schema));
  writeFileSync(
    join(directory, "parts/settings.json"),
    JSON.stringify({
      type: "object",
      additionalProperties: { type: "integer" },
      "x-tombi-table-keys-order": "ascending",
    }),
  );
  const config = `[[schemas]]\npath = "schema.json"\ninclude = ["example.toml"]\n`;
  writeFileSync(join(directory, "tombi.toml"), config);
  writeFileSync(
    join(directory, "dprint.json"),
    JSON.stringify({ plugins: [plugin], incremental: false, toml: { configFile: "tombi.toml" } }),
  );
  const sorted = compare("schema property order, array of tables and relative $ref", "example.toml");
  assert.ok(sorted.indexOf("[[ignores]]") < sorted.indexOf("[local-actions]"), sorted);
  assert.ok(sorted.indexOf("a = 2") < sorted.indexOf("z = 1"), sorted);

  writeFileSync(join(directory, "tombi.toml"), `${config}\n[schema.format.rules.table-keys-order]\nenabled = false\n`);
  const unsorted = compare("schema sorting disabled in tombi.toml", "example.toml");
  assert.ok(unsorted.indexOf("[local-actions]") < unsorted.indexOf("[[ignores]]"), unsorted);

  writeFileSync(join(directory, "tombi.toml"), config);
  assert.equal(compare("schema sorting re-enabled", "example.toml"), sorted);

  writeFileSync(
    join(directory, "parts/settings.json"),
    JSON.stringify({
      type: "object",
      additionalProperties: { type: "integer" },
      "x-tombi-table-keys-order": "descending",
    }),
  );
  const changed = compare("changed referenced schema", "example.toml");
  assert.ok(changed.indexOf("z = 1") < changed.indexOf("a = 2"), changed);

  compare("unmatched source file", "unmatched.toml");
  mkdirSync(join(directory, "nested"));
  const nested = compare(
    "relative schema directive in a subdirectory",
    "nested/other.toml",
    `#:schema ../schema.json\n\n${source}`,
  );
  assert.ok(nested.indexOf("[[ignores]]") < nested.indexOf("[local-actions]"), nested);

  mkdirSync(join(directory, ".github"));
  cpSync(join(fixtures, "actionlint.schema.json"), join(directory, "actionlint.schema.json"));
  writeFileSync(
    join(directory, "tombi.toml"),
    `${config}\n[[schemas]]\npath = "actionlint.schema.json"\ninclude = [".github/actionlint.toml"]\n`,
  );
  compare(
    "actionlint config with optional sections",
    ".github/actionlint.toml",
    readFileSync(join(fixtures, "actionlint.toml"), "utf8"),
  );

  if (process.env.ACTIONLINT_PROJECT) {
    const project = process.env.ACTIONLINT_PROJECT;
    cpSync(join(project, "schemas"), join(directory, "schemas"), { recursive: true });
    cpSync(join(project, "actionlint.schema.json"), join(directory, "actionlint.schema.json"));
    cpSync(join(project, "tombi.toml"), join(directory, "tombi.toml"));
    compare(
      "actionlint's tombi.toml, schema and configuration",
      ".github/actionlint.toml",
      readFileSync(join(project, ".github/actionlint.toml"), "utf8"),
    );
  }
} finally {
  rmSync(directory, { recursive: true, force: true });
}
