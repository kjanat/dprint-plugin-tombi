import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const directory = await mkdtemp(join(tmpdir(), "tombi-remote-config-"));
const plugin = resolve(process.argv[2] ?? "target/wasm32-unknown-unknown/wasm-release/dprint_plugin_tombi.wasm");
const requests = new Map();
const routes = new Map();
const redirects = new Map([["/config.json", "/team/dprint.json"]]);
const server = createServer((request, response) => {
  const path = request.url;
  requests.set(path, (requests.get(path) ?? 0) + 1);
  if (redirects.has(path)) {
    response.writeHead(302, { location: redirects.get(path) });
    response.end();
    return;
  }
  const text = routes.get(path);
  response.writeHead(text === undefined ? 404 : 200, { "content-type": "text/plain" });
  response.end(text ?? "not found");
});
await new Promise((resolve) => server.listen(0, "localhost", resolve));
const address = server.address();
assert.ok(address && typeof address !== "string");
const origin = `http://localhost:${address.port}`;
const kprint = process.env.KPRINT_BIN ?? "kprint";
const tombi = process.env.TOMBI_BIN ?? "tombi";
const source = "name=\"demo\"\nvalues=[1,2,3,4,5,6]\n";

async function run(binary, args, input = source, successful = true) {
  const child = spawn(binary, args, {
    cwd: directory,
    env: {
      ...process.env,
      NO_COLOR: "1",
      DPRINT_CACHE_DIR: join(directory, ".cache"),
      TOMBI_CACHE_HOME: join(directory, ".tombi-cache"),
      TOMBI_SCHEMA_TRUSTED_HOSTS: "localhost",
    },
    stdio: ["pipe", "pipe", "pipe"],
  });
  let stdout = "";
  let stderr = "";
  child.stdout.setEncoding("utf8").on("data", (data) => {
    stdout += data;
  });
  child.stderr.setEncoding("utf8").on("data", (data) => {
    stderr += data;
  });
  child.stdin.end(input);
  const timer = setTimeout(() => child.kill(), 300_000);
  try {
    const code = await new Promise((resolve, reject) => {
      child.once("error", reject);
      child.once("close", resolve);
    });
    if (successful) assert.equal(code, 0, `${binary} ${args.join(" ")}\n${stdout}\n${stderr}`);
    else assert.notEqual(code, 0, "Expected invalid configuration to fail");
    return { stdout, stderr };
  } finally {
    clearTimeout(timer);
  }
}

try {
  routes.set("/team/dprint.json", JSON.stringify({ extends: "../shared/base.json" }));
  routes.set("/shared/base.json", JSON.stringify({ toml: { configFile: "./tombi.toml" } }));
  routes.set("/shared/tombi.toml", "extends = ['../defaults/tombi.toml']\n[format.rules]\nline-width = 24\n");
  routes.set("/defaults/tombi.toml", "[format.rules]\nstring-quote-style = 'single'\nline-width = 120\n");
  await writeFile(join(directory, "dprint.json"), JSON.stringify({ plugins: [plugin], extends: `${origin}/config.json`, incremental: false, useTabs: true }));
  await writeFile(join(directory, "tombi.toml"), `extends = ['${origin}/shared/tombi.toml']\n[format.rules]\nindent-style = 'tab'\n`);
  const filename = join(directory, "example.toml");
  await writeFile(filename, source);
  const expected = (await run(tombi, ["format", "--stdin-filename", filename, "-"])).stdout;
  const actual = (await run(kprint, ["fmt", "--stdin", filename])).stdout;
  assert.equal(actual, expected, "Remote Tombi inheritance differs from the CLI");
  assert.match(actual, /name = 'demo'/);
  assert.match(actual, /values = \[\n/);
  assert.match(actual, /\n\t1,/);
  const before = new Map(requests);
  assert.equal((await run(kprint, ["fmt", "--stdin", filename])).stdout, actual);
  assert.deepEqual(requests, before, "The second invocation should use kprint's download cache");
  console.log("Passed: remote config, redirect, relative configFile, remote extends, precedence, CLI parity and cached second invocation");

  routes.set("/sorting/tombi.toml", "extends = ['../schemas/tombi.toml']\n");
  routes.set("/schemas/tombi.toml", "[[schemas]]\npath = './root.json'\ninclude = ['example.toml']\n");
  routes.set(
    "/schemas/root.json",
    JSON.stringify({
      type: "object",
      properties: { settings: { $ref: "parts/settings.json" } },
    }),
  );
  routes.set(
    "/schemas/parts/settings.json",
    JSON.stringify({
      type: "object",
      additionalProperties: { type: "integer" },
      "x-tombi-table-keys-order": "ascending",
    }),
  );
  await writeFile(
    join(directory, "dprint.json"),
    JSON.stringify({ plugins: [plugin], toml: { configFile: `${origin}/sorting/tombi.toml` }, incremental: false }),
  );
  await writeFile(join(directory, "tombi.toml"), `[[schemas]]\npath = '${origin}/schemas/root.json'\ninclude = ['example.toml']\n`);
  const unsorted = "[settings]\nz=1\na=2\n";
  const sorted = (await run(kprint, ["fmt", "--stdin", filename], unsorted)).stdout;
  assert.equal(sorted, (await run(tombi, ["format", "--stdin-filename", filename, "-"], unsorted)).stdout);
  assert.ok(sorted.indexOf("a = 2") < sorted.indexOf("z = 1"), sorted);
  console.log("Passed: inherited relative schema URL, relative $ref and schema sorting parity");

  redirects.set("/alias/tombi.toml", "/sorting/tombi.toml");
  routes.set("/redirect/tombi.toml", "extends = ['../alias/tombi.toml']\n");
  await writeFile(
    join(directory, "dprint.json"),
    JSON.stringify({ plugins: [plugin], toml: { configFile: `${origin}/redirect/tombi.toml` }, incremental: false }),
  );
  assert.equal((await run(kprint, ["fmt", "--stdin", filename], unsorted)).stdout, sorted);
  console.log("Passed: nested redirect resolves relative references from the destination URL");

  routes.set("/custom/settings.toml", "[tool.tombi]\nextends = ['../shared/tombi.toml']\n");
  await writeFile(
    join(directory, "dprint.json"),
    JSON.stringify({ plugins: [plugin], toml: { configFile: `${origin}/custom/settings.toml#tool.tombi` }, incremental: false }),
  );
  assert.equal((await run(kprint, ["fmt", "--stdin", filename])).stdout, expected.replaceAll("\t", "  "));
  console.log("Passed: selected section in a remote config with a custom filename");

  await mkdir(join(directory, ".config"));
  await mkdir(join(directory, "base"));
  await rm(join(directory, "tombi.toml"));
  await writeFile(join(directory, ".config/tombi.toml"), "extends = ['base/tombi.toml']\n");
  await writeFile(join(directory, "base/tombi.toml"), `extends = ['${origin}/shared/tombi.toml']\n`);
  await writeFile(join(directory, "dprint.json"), JSON.stringify({ plugins: [plugin], incremental: false }));
  assert.equal((await run(kprint, ["fmt", "--stdin", filename])).stdout, (await run(tombi, ["format", "--stdin-filename", filename, "-"])).stdout);
  console.log("Passed: .config/tombi.toml uses the project directory for relative extends");

  for (
    const [name, config, pattern] of [
      ["missing", "extends = ['../absent/tombi.toml']\n", /404|not found/i],
      ["cycle", "extends = ['./tombi.toml']\n", /cycl/i],
      ["invalid", "extends = 42\n", /extends|array/i],
      ["local", "extends = ['file:///private/tombi.toml']\n", /cannot reference|HTTP/i],
    ]
  ) {
    routes.set(`/${name}/tombi.toml`, config);
    await writeFile(
      join(directory, "dprint.json"),
      JSON.stringify({ plugins: [plugin], toml: { configFile: `${origin}/${name}/tombi.toml` }, incremental: false }),
    );
    const failure = await run(kprint, ["fmt", "--stdin", filename], source, false);
    assert.match(failure.stderr, pattern, name);
    assert.equal(failure.stdout, "", "Invalid configuration must not produce formatted output");
  }
  console.log("Passed: missing references, cycles, invalid extends and remote-to-local references fail");
} finally {
  server.closeAllConnections();
  await new Promise((resolve) => server.close(resolve));
  await rm(directory, { recursive: true, force: true });
}
