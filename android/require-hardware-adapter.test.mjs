import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const androidDir = dirname(fileURLToPath(import.meta.url));
const checker = join(androidDir, "require-hardware-adapter.mjs");

test("accepts a host MoltenVK adapter", () => {
  const result = check({
    name: "Apple M4 Pro",
    backend: "Vulkan",
    deviceType: "IntegratedGpu",
    driver: "MoltenVK",
    driverInfo: "1.4.0",
  });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /PHYSBOX5_HARDWARE_ADAPTER_SUCCESS/);
});

test("accepts nested hardware benchmark adapters", () => {
  const result = check({
    lanes: [
      { adapter: { name: "Adreno", backend: "Vulkan", deviceType: "IntegratedGpu", driver: "Qualcomm" } },
      { adapter: { name: "Adreno", backend: "Vulkan", deviceType: "IntegratedGpu", driver: "Qualcomm" } },
    ],
  });
  assert.equal(result.status, 0, result.stderr);
});

test("rejects SwiftShader by identity", () => {
  const result = check({
    adapter: {
      name: "SwiftShader Device (LLVM 10.0.0)",
      backend: "Vulkan",
      deviceType: "Unknown",
      driver: "SwiftShader driver",
    },
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /rejected software\/CPU adapter/);
});

test("rejects generic CPU adapters", () => {
  const result = check({
    name: "Generic Vulkan",
    backend: "Vulkan",
    deviceType: "Cpu",
    driver: "unknown",
  });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /deviceType=Cpu/);
});

test("rejects missing adapter diagnostics", () => {
  const result = check({ schemaVersion: 1, lanes: [] });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /adapter diagnostics are missing/);
});

test("rejects malformed adapter diagnostics", () => {
  const result = check({ schemaVersion: 1, adapter: {} });
  assert.equal(result.status, 1);
  assert.match(result.stderr, /adapter diagnostics are missing/);
});

test("maintained AVD launchers force host graphics and invoke the guard", () => {
  for (const file of ["validate-avd.sh", "benchmark-gpu.sh", "benchmark-bodies.sh"]) {
    const source = readFileSync(join(androidDir, file), "utf8");
    assert.match(source, /-gpu host/, `${file} must force host graphics`);
    assert.match(source, /require-hardware-adapter\.mjs/, `${file} must validate the reported adapter`);
  }
});

function check(payload) {
  return spawnSync(process.execPath, [checker], {
    encoding: "utf8",
    input: `${JSON.stringify(payload)}\n`,
  });
}
