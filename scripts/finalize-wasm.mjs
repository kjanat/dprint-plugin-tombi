import assert from "node:assert/strict";
import { mkdirSync, readFileSync, renameSync, writeFileSync } from "node:fs";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import binaryen from "binaryen";

const browserExport = (name) => name.startsWith("__wbindgen_") || name.startsWith("__externref_");
const exportSignature = (module) => WebAssembly.Module.exports(module)
  .map(({ name, kind }) => `${kind}:${name}`).sort();

// wasm-bindgen retains browser entry points as exports, even when the dprint
// call graph never reaches them (for example through Chrono's default features).
// Remove only those foreign entry points, then let Binaryen prove code dead.
// Never stub imports or delete instructions from a reachable formatter path.
export function finalizeWasm(bytes) {
  const input = new WebAssembly.Module(bytes);
  const exports = WebAssembly.Module.exports(input);
  const module = binaryen.readBinary(bytes);
  let output;
  try {
    // The passes below only delete unreachable elements; they do not introduce
    // instructions requiring new features. Both V8 and dprint validate output.
    module.setFeatures(binaryen.Features.All);
    for (const entry of exports) {
      if (browserExport(entry.name)) module.removeExport(entry.name);
    }
    module.runPasses(["remove-unused-module-elements", "dce", "vacuum", "remove-unused-module-elements"]);
    assert.ok(module.validate(), "Finalized Wasm failed Binaryen validation");
    output = module.emitBinary();
  } finally {
    module.dispose();
  }
  const compiled = new WebAssembly.Module(output);
  assert.deepEqual(
    exportSignature(compiled),
    exports.filter(({ name }) => !browserExport(name)).map(({ name, kind }) => `${kind}:${name}`).sort(),
    "Finalization changed a non-browser export",
  );
  assert.deepEqual(
    WebAssembly.Module.imports(compiled).filter(({ module }) => module !== "dprint"),
    [],
    "A reachable non-dprint host dependency remains; fix its source rather than stubbing it",
  );
  return output;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const input = resolve(process.argv[2] ?? "target/wasm32-unknown-unknown/wasm-release/dprint_plugin_tombi.wasm");
  const output = resolve(process.argv[3] ?? "dist/plugin.wasm");
  assert.notEqual(input, output, "Do not overwrite Cargo's raw build artifact");
  const bytes = finalizeWasm(readFileSync(input));
  mkdirSync(dirname(output), { recursive: true });
  writeFileSync(`${output}.tmp`, bytes);
  renameSync(`${output}.tmp`, output);
  console.log(`Wrote ${output} (${bytes.length} bytes); imports are dprint-only.`);
}
