import { mkdtemp, readFile, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join, relative, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
const output = await mkdtemp(join(tmpdir(), "physbox5-comparator-off-"));

try {
  run(process.execPath, [
    resolve(root, "scripts/build-web.mjs"),
    "--release",
    "--no-box2d-comparator",
    "--output",
    output,
  ]);
  run("cargo", [
    "test",
    "--quiet",
    "-p",
    "physbox-engine",
    "--lib",
    "--no-default-features",
    "feature_off_frame_has_no_comparator_selection_or_runtime_state",
  ]);

  const dependencyTree = capture("cargo", [
    "tree",
    "-p",
    "physbox-web",
    "--target",
    "wasm32-unknown-unknown",
    "--no-default-features",
  ]);
  if (dependencyTree.includes("physbox-box2d-comparator")) {
    throw new Error("feature-off web dependency graph still contains the Box2D comparator");
  }

  const paths = await files(output);
  const relativePaths = paths.map((path) => relative(output, path));
  if (relativePaths.some((path) => /box2d|comparator/i.test(path))) {
    throw new Error(`feature-off web package contains comparator paths: ${relativePaths.join(", ")}`);
  }

  const appSource = await readFile(join(output, "app.js"), "utf8");
  for (const forbidden of ["box2dComparator", "box2d-comparator", "loadBox2dComparator"]) {
    if (appSource.includes(forbidden)) {
      throw new Error(`feature-off browser bridge retains ${forbidden}`);
    }
  }

  const wasm = await readFile(join(output, "pkg", "physbox_web_bg.wasm"));
  const wasmText = wasm.toString("latin1");
  for (const forbidden of [
    "3a4f0da8374af61293a03021c9a0b3ebcfe67948",
    "PHYSBOX5_BOX2D_REVISION",
    "pb5_box2d",
    "physbox_box2d",
    "BOX2D PUBLIC",
  ]) {
    if (wasmText.includes(forbidden)) {
      throw new Error(`feature-off WebAssembly retains comparator marker ${forbidden}`);
    }
  }

  console.log(
    `PHYSBOX5_BOX2D_FEATURE_OFF_SUCCESS files=${relativePaths.length} dependency_absent=true runtime_markers_absent=true`,
  );
} finally {
  await rm(output, { recursive: true, force: true });
}

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
  }
}

function capture(command, args) {
  const result = spawnSync(command, args, { cwd: root, encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    throw new Error(`${command} failed: ${result.stderr}`);
  }
  return result.stdout;
}

async function files(directory) {
  const outputPaths = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) outputPaths.push(...await files(path));
    else outputPaths.push(path);
  }
  return outputPaths;
}
