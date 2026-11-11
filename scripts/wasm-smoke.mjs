import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const plugin = resolve(process.argv[2] ?? "target/wasm32-unknown-unknown/wasm-release/dprint_plugin_tombi.wasm");
const module = new WebAssembly.Module(readFileSync(plugin));
const foreign = WebAssembly.Module.imports(module).filter((entry) => entry.module !== "dprint");
assert.deepEqual(foreign, [], "The plugin must not import JavaScript, WASI, or other non-dprint APIs");
assert.ok(WebAssembly.Module.exports(module).some(({ name }) => name === "dprint_plugin_version_4"));
const noHostFormatting = () => {
  throw new Error("TOML must not request host formatting");
};
const wasm = new WebAssembly.Instance(module, {
  dprint: {
    host_has_cancelled: () => 0,
    host_format: noHostFormatting,
    host_write_buffer: noHostFormatting,
    host_get_formatted_text: noHostFormatting,
    host_get_error_text: noHostFormatting,
  },
}).exports;
function send(text) {
  const bytes = new TextEncoder().encode(text);
  const pointer = wasm.clear_shared_bytes(bytes.length);
  new Uint8Array(wasm.memory.buffer, pointer, bytes.length).set(bytes);
}
function receive(length) {
  return new TextDecoder().decode(new Uint8Array(wasm.memory.buffer, wasm.get_shared_bytes_ptr(), length));
}
function formatInInstance(settings, source) {
  send(JSON.stringify({ global: {}, plugin: settings }));
  wasm.register_config(1);
  assert.deepEqual(JSON.parse(receive(wasm.get_config_diagnostics(1))), []);
  send("C:\\project\\example.toml");
  wasm.set_file_path();
  send(source);
  const result = wasm.format(1);
  if (result === 2) throw new Error(receive(wasm.get_error_text()));
  assert.ok(result === 0 || result === 1);
  return result === 0 ? source : receive(wasm.get_formatted_text());
}
const sortingSchema = JSON.stringify({
  type: "object",
  additionalProperties: { type: "integer" },
  "x-tombi-table-keys-order": "ascending",
});
assert.equal(
  formatInInstance({
    $configFile: {
      path: "tombi.toml",
      content: JSON.stringify({ schema: { enabled: false } }),
      files: { "https://example.com/schema": "not JSON" },
      fileSources: { "https://example.com/schema": "https://example.com/schema.json" },
    },
  }, "a=1\n"),
  "a = 1\n",
  "Disabled schemas must not parse malformed redirected resources",
);
const inherited = {
  $configFile: {
    path: "pyproject.toml#tool.tombi",
    absolutePath: "C:\\project\\pyproject.toml",
    source: "file:///C:/project/pyproject.toml",
    text: "[tool.tombi]\nextends = ['base/tombi.toml']\n",
    content: JSON.stringify({ extends: ["base/tombi.toml"] }),
    documents: {
      "file:///C:/project/base/tombi.toml": {
        source: "file:///C:/project/base/tombi.toml",
        text: "[format.rules]\nstring-quote-style = 'single'\n",
      },
    },
    files: { "C:\\project\\first.json": sortingSchema, "C:\\project\\second.json": sortingSchema },
  },
};
assert.equal(formatInInstance(inherited, "name=\"demo\"\n"), "name = 'demo'\n", "Windows pyproject.toml inheritance");
for (const name of ["first", "second"]) {
  assert.equal(
    formatInInstance(inherited, `#:schema ${name}.json\n\nz=1\na=2\n`),
    `#:schema ${name}.json\n\na = 2\nz = 1\n`,
    "Resolving config again must reinstall uncached schemas in the same Wasm instance",
  );
}
console.log("WASM Windows pyproject inheritance and schema reload passed.");
const root = dirname(dirname(fileURLToPath(import.meta.url)));
const dir = mkdtempSync(join(tmpdir(), "dprint-tombi-"));
const configPath = join(dir, "dprint.json");
const config = {
  plugins: [plugin],
  incremental: false,
  includes: ["**/*.toml", "**/*.toml-extra"],
  toml: {
    stringQuoteStyle: "single",
    associations: ["**/*.toml-extra", "!**/excluded.toml"],
    overrides: [{ files: "**/override.toml", stringQuoteStyle: "double" }],
  },
};
function run(command, successful = true) {
  const result = spawnSync(process.env.DPRINT_BIN ?? "dprint", [...command, "--config", configPath], {
    cwd: dir,
    encoding: "utf8",
    timeout: 60_000,
    env: { ...process.env, DPRINT_CACHE_DIR: join(dir, ".cache") },
  });
  if (result.error) throw result.error;
  assert.equal(result.signal, null, `dprint was terminated: ${result.signal}`);
  if (successful) assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
  else assert.notEqual(result.status, 0, "Expected dprint to reject this input/configuration");
  return result;
}
function contents(name) {
  return readFileSync(join(dir, name), "utf8");
}
function writeConfig() {
  writeFileSync(configPath, JSON.stringify(config));
}
try {
  writeConfig();
  writeFileSync(join(dir, "fixture.toml"), readFileSync(join(root, "tests/fixtures/source.toml")));
  writeFileSync(join(dir, "normal.toml"), "name=\"demo\"\n");
  writeFileSync(join(dir, "extra.toml-extra"), "name=\"demo\"\n");
  writeFileSync(join(dir, "override.toml"), "name='demo'\n");
  writeFileSync(join(dir, "excluded.toml"), "a=1\n");
  const disabled = "#:tombi format.disabled = true\n\na=1\n";
  writeFileSync(join(dir, "disabled.toml"), disabled);
  writeFileSync(
    join(dir, "dates.toml"),
    "offset=1979-05-27T07:32:00Z\nlocal=1979-05-27T07:32:00\ndate=1979-05-27\ntime=07:32:00\n",
  );
  run(["check"], false);
  run(["fmt"]);
  assert.equal(contents("fixture.toml"), readFileSync(join(root, "tests/fixtures/expected.toml"), "utf8"));
  assert.equal(contents("normal.toml"), "name = 'demo'\n");
  assert.equal(contents("extra.toml-extra"), "name = 'demo'\n");
  assert.equal(contents("override.toml"), "name = \"demo\"\n");
  assert.equal(contents("excluded.toml"), "a=1\n");
  assert.equal(contents("disabled.toml"), disabled);
  assert.equal(
    contents("dates.toml"),
    "offset = 1979-05-27T07:32:00Z\nlocal = 1979-05-27T07:32:00\ndate = 1979-05-27\ntime = 07:32:00\n",
  );
  run(["check"]);
  run(["fmt"]);
  run(["check"]);

  writeFileSync(join(dir, "invalid.toml"), "a = [");
  run(["fmt"], false);
  assert.equal(contents("invalid.toml"), "a = [");
  rmSync(join(dir, "invalid.toml"));
  config.toml.indentStyle = "banana";
  writeConfig();
  const invalid = run(["check"], false);
  assert.match(invalid.stdout + invalid.stderr, /indentStyle/);
  delete config.toml.indentStyle;
  config.toml.misspelledOption = true;
  writeConfig();
  const unknown = run(["check"], false);
  assert.match(unknown.stdout + unknown.stderr, /misspelledOption/);
  delete config.toml.misspelledOption;
  writeConfig();

  // Tombi's built-in schemas for tombi.toml and Cargo.toml say TOML v1.1.0,
  // which keeps a multi-line inline table with a trailing comma, as
  // `tombi format` does. A configured schema comes first: Cargo reads TOML
  // 1.0, so a tombi.toml can say so for Cargo.toml.
  for (const name of readdirSync(dir).filter((name) => /\.toml(-extra)?$/.test(name))) rmSync(join(dir, name));
  // single quotes, as configured above
  const multiLine = "catalog = {\n  paths = [\n    'a',\n    'b',\n  ],\n}\n";
  const singleLine = "catalog = { paths = [\n  'a',\n  'b',\n] }\n";
  writeFileSync(join(dir, "tombi.toml"), multiLine);
  writeFileSync(join(dir, "Cargo.toml"), multiLine);
  run(["fmt"]);
  assert.equal(contents("tombi.toml"), multiLine);
  assert.equal(contents("Cargo.toml"), multiLine);
  config.toml.schemas = [
    { path: "https://www.schemastore.org/cargo.json", include: ["Cargo.toml", "**/Cargo.toml"], tomlVersion: "v1.0.0" },
  ];
  writeConfig();
  run(["fmt"]);
  assert.equal(contents("tombi.toml"), multiLine);
  assert.equal(contents("Cargo.toml"), singleLine);
  run(["check"]);

  rmSync(join(dir, "tombi.toml"));
  rmSync(join(dir, "Cargo.toml"));
  delete config.toml.schemas;
  config.toml.$configFile = {
    path: "tombi.toml",
    absolutePath: join(dir, "tombi.toml"),
    content: JSON.stringify({ schemas: [{ path: "schema.json", include: ["ordered.toml"] }] }),
    files: {
      [join(dir, "schema.json")]: JSON.stringify({
        type: "object",
        properties: {
          first: { $ref: "fields.json" },
          entries: { type: "array", items: { type: "object" } },
          last: { type: "object" },
        },
        "x-tombi-table-keys-order": { properties: "schema" },
      }),
      [join(dir, "fields.json")]: JSON.stringify({ type: "object", "x-tombi-table-keys-order": "ascending" }),
    },
  };
  writeConfig();
  writeFileSync(join(dir, "ordered.toml"), "[last]\nx = 0\n\n[[entries]]\nx = 0\n\n[first]\nz = 1\na = 2\n");
  run(["fmt"]);
  assert.equal(contents("ordered.toml"), "[first]\na = 2\nz = 1\n\n[[entries]]\nx = 0\n\n[last]\nx = 0\n");
  run(["check"]);
  console.log(
    "WASM imports, formatting, dates, directives, idempotence, associations, overrides, diagnostics, and schemas passed.",
  );
} finally {
  rmSync(dir, { recursive: true, force: true });
}
