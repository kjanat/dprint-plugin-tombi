import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const plugin = resolve(process.argv[2] ?? "target/wasm32-unknown-unknown/wasm-release/dprint_plugin_tombi.wasm");
const module = new WebAssembly.Module(readFileSync(plugin));
const foreign = WebAssembly.Module.imports(module).filter((entry) => entry.module !== "dprint");
assert.deepEqual(foreign, [], "The plugin must not import JavaScript, WASI, or other non-dprint APIs");
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
function contents(name) { return readFileSync(join(dir, name), "utf8"); }
function writeConfig() { writeFileSync(configPath, JSON.stringify(config)); }
try {
  writeConfig();
  writeFileSync(join(dir, "fixture.toml"), readFileSync(join(root, "tests/fixtures/source.toml")));
  writeFileSync(join(dir, "normal.toml"), 'name="demo"\n');
  writeFileSync(join(dir, "extra.toml-extra"), 'name="demo"\n');
  writeFileSync(join(dir, "override.toml"), "name='demo'\n");
  writeFileSync(join(dir, "excluded.toml"), "a=1\n");
  run(["check"], false);
  run(["fmt"]);
  assert.equal(contents("fixture.toml"), readFileSync(join(root, "tests/fixtures/expected.toml"), "utf8"));
  assert.equal(contents("normal.toml"), "name = 'demo'\n");
  assert.equal(contents("extra.toml-extra"), "name = 'demo'\n");
  assert.equal(contents("override.toml"), 'name = "demo"\n');
  assert.equal(contents("excluded.toml"), "a=1\n");
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
  console.log("WASM imports, formatting, idempotence, associations, overrides, and diagnostics passed.");
} finally {
  rmSync(dir, { recursive: true, force: true });
}
