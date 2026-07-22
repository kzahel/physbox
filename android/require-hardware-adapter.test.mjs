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

test("display-less Linux AVD launchers stay headless with guarded host graphics", () => {
  const toolchain = readFileSync(join(androidDir, "toolchain.sh"), "utf8");
  assert.match(toolchain, /physbox5_android_configure_emulator_display/);
  assert.match(toolchain, /physbox5_host_import_graphical_session force/);
  assert.match(toolchain, /physbox5_host_x11_available/);
  assert.match(toolchain, /command -v xvfb-run/);
  assert.match(toolchain, /PHYSBOX5_ANDROID_EMULATOR_PREFIX=\(xvfb-run -a\)/);
  assert.match(toolchain, /PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS=\(-no-window\)/);

  for (const file of ["validate-avd.sh", "benchmark-gpu.sh", "benchmark-bodies.sh"]) {
    const source = readFileSync(join(androidDir, file), "utf8");
    assert.match(source, /-gpu host/, `${file} must force host graphics`);
    assert.match(source, /physbox5_android_configure_emulator_display/, `${file} must configure display-less Linux`);
    assert.match(source, /PHYSBOX5_ANDROID_EMULATOR_PREFIX\[@\]/, `${file} must use the shared emulator prefix`);
    assert.match(source, /PHYSBOX5_ANDROID_EMULATOR_DISPLAY_ARGS\[@\]/, `${file} must use the shared display arguments`);
    assert.match(source, /require-hardware-adapter\.mjs/, `${file} must validate the reported adapter`);
  }
});

test("ordinary AVD launchers bound shutdown and reap their emulator child", () => {
  const toolchain = readFileSync(join(androidDir, "toolchain.sh"), "utf8");
  assert.match(toolchain, /physbox5_android_stop_emulator/);
  assert.match(toolchain, /emu kill/);
  assert.match(toolchain, /wait "\$emulator_pid"/);
  assert.match(toolchain, /kill -TERM "\$emulator_pid"/);
  assert.match(toolchain, /kill -KILL "\$emulator_pid"/);

  for (const file of [
    "validate-avd.sh",
    "benchmark-gpu.sh",
    "benchmark-bodies.sh",
    "trace-settling.sh",
  ]) {
    const source = readFileSync(join(androidDir, file), "utf8");
    assert.match(source, /EMULATOR_PID=\$!/, `${file} must retain its emulator child PID`);
    assert.match(
      source,
      /physbox5_android_stop_emulator "\$ADB" "\$SERIAL" "\$EMULATOR_PID" "\$AVD_NAME"/,
      `${file} must use bounded shared shutdown`,
    );
  }
});

test("AVD sleep capture uses the engine menu and verifies effective state", () => {
  const source = readFileSync(join(androidDir, "validate-avd.sh"), "utf8");
  assert.match(source, /PHYSBOX5_ANDROID_SLEEP_TOGGLE/);
  assert.match(source, /frame\.sceneContract\.w === false && frame\.sceneContract\.y === false/);
  assert.match(source, /\[62, 50, 300, 104, 192, 331\]/);
  assert.match(source, /frame\.sceneContract\.w === true && frame\.sceneContract\.y === true/);
  assert.match(source, /frame\.scenario\.complete === true && frame\.scenario\.passed === true/);
  assert.match(source, /sleep-request-on-menu/);
});

test("APK validation runners stop the product app on every exit", () => {
  const toolchain = readFileSync(join(androidDir, "toolchain.sh"), "utf8");
  assert.match(toolchain, /PHYSBOX5_ANDROID_APP_ID="com\.kzahel\.physbox5"/);
  assert.match(toolchain, /physbox5_android_stop_product_app\(\)/);
  assert.match(toolchain, /shell am force-stop "\$PHYSBOX5_ANDROID_APP_ID"/);

  for (const file of ["validate-device.sh", "validate-avd.sh", "validate-pacing.sh"]) {
    const source = readFileSync(join(androidDir, file), "utf8");
    assert.match(
      source,
      /physbox5_android_stop_product_app "\$ADB" "\$SERIAL"/,
      `${file} must force-stop the product app during cleanup`,
    );
    assert.match(source, /trap cleanup EXIT/, `${file} must clean up on every exit`);
    assert.match(source, /trap 'exit 130' INT TERM/, `${file} must clean up after interruption`);
  }

  const pacing = readFileSync(join(androidDir, "validate-pacing.sh"), "utf8");
  assert.doesNotMatch(pacing, /restore_product_launch/);
});

function check(payload) {
  return spawnSync(process.execPath, [checker], {
    encoding: "utf8",
    input: `${JSON.stringify(payload)}\n`,
  });
}
