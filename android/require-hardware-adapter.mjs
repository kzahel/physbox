#!/usr/bin/env node

process.stdin.setEncoding("utf8");
let input = "";
for await (const chunk of process.stdin) input += chunk;
let payload;
try {
  payload = JSON.parse(input);
} catch (error) {
  fail(`adapter diagnostics are not valid JSON: ${error.message}`);
}

const adapters = collectAdapters(payload);
if (adapters.length === 0) {
  fail("adapter diagnostics are missing");
}

for (const adapter of adapters) {
  const identity = [adapter.name, adapter.driver, adapter.driverInfo]
    .filter((value) => typeof value === "string")
    .join(" ");
  const deviceType = typeof adapter.deviceType === "string" ? adapter.deviceType : "";
  const softwareIdentity = /swiftshader|llvmpipe|lavapipe|software(?:\s+rasterizer)?/i.test(identity);
  if (deviceType.toLowerCase() === "cpu" || softwareIdentity) {
    fail(
      `AVD hardware-GPU evidence rejected software/CPU adapter: ${adapter.name ?? "unknown"}`
      + ` (deviceType=${deviceType || "unknown"}, driver=${adapter.driver ?? "unknown"})`,
    );
  }
}

const names = [...new Set(adapters.map((adapter) => adapter.name ?? "unknown"))];
console.log(`PHYSBOX5_HARDWARE_ADAPTER_SUCCESS adapters=${adapters.length} names=${names.join(",")}`);

function collectAdapters(value) {
  const found = [];
  const seen = new Set();

  function add(adapter) {
    const key = JSON.stringify(adapter);
    if (!seen.has(key)) {
      seen.add(key);
      found.push(adapter);
    }
  }

  function visit(current, key = null) {
    if (current === null || typeof current !== "object") return;
    if (key === "adapter" && isAdapter(current)) add(current);
    for (const [childKey, child] of Object.entries(current)) visit(child, childKey);
  }

  if (isAdapter(value)) add(value);
  visit(value);
  return found;
}

function isAdapter(value) {
  return value !== null
    && typeof value === "object"
    && !Array.isArray(value)
    && typeof value.name === "string"
    && (typeof value.backend === "string" || typeof value.deviceType === "string");
}

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}
