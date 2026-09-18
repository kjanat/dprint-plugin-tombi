import assert from "node:assert/strict";
import test from "node:test";
import binaryen from "binaryen";
import { finalizeWasm } from "./finalize-wasm.mjs";

function wasm(text) {
  const module = binaryen.parseText(text);
  try { return module.emitBinary(); }
  finally { module.dispose(); }
}

test("unreachable browser entry points disappear without changing host exports", () => {
  const output = finalizeWasm(wasm(`(module
    (import "__wbindgen_placeholder__" "browser" (func $browser))
    (func $glue (call $browser))
    (export "__wbindgen_glue" (func $glue))
    (func $format (result i32) (i32.const 42))
    (export "format" (func $format))
  )`));
  const module = new WebAssembly.Module(output);
  assert.deepEqual(WebAssembly.Module.imports(module), []);
  assert.equal(new WebAssembly.Instance(module).exports.format(), 42);
});

test("reachable dprint imports are preserved", () => {
  const output = finalizeWasm(wasm(`(module
    (import "dprint" "host" (func $host (result i32)))
    (func $format (result i32) (call $host))
    (export "format" (func $format))
  )`));
  const instance = new WebAssembly.Instance(new WebAssembly.Module(output), { dprint: { host: () => 7 } });
  assert.equal(instance.exports.format(), 7);
});

test("multiple imports do not opt into experimental compact-import encoding", () => {
  const output = finalizeWasm(wasm(`(module
    (import "dprint" "first" (func $first))
    (import "dprint" "second" (func $second))
    (import "dprint" "third" (func $third))
    (func $format (call $first) (call $second) (call $third))
    (export "format" (func $format))
  )`));
  const module = new WebAssembly.Module(output);
  assert.equal(WebAssembly.Module.imports(module).length, 3);
});

test("reachable browser calls fail closed instead of being replaced or removed", () => {
  assert.throws(() => finalizeWasm(wasm(`(module
    (import "__wbindgen_placeholder__" "browser" (func $browser))
    (func $glue (call $browser))
    (export "__wbindgen_glue" (func $glue))
    (func $format (call $glue))
    (export "format" (func $format))
  )`)), /reachable non-dprint/);
});

test("browser calls in a start function also fail closed", () => {
  assert.throws(() => finalizeWasm(wasm(`(module
    (import "__wbindgen_placeholder__" "browser" (func $browser))
    (func $start (call $browser))
    (start $start)
  )`)), /reachable non-dprint/);
});
