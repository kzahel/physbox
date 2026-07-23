import { cp, mkdir, readFile, rm, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
let release = false;
let solverLab = false;
let box2dComparator = true;
let output = join(root, "target", "web");
for (let index = 2; index < process.argv.length; index += 1) {
  const argument = process.argv[index];
  if (argument === "--release") release = true;
  else if (argument === "--solver-lab") solverLab = true;
  else if (argument === "--no-box2d-comparator") box2dComparator = false;
  else if (argument === "--output") {
    if (!process.argv[index + 1]) throw new Error("--output requires a path");
    output = resolve(root, process.argv[++index]);
  } else throw new Error(`unknown argument: ${argument}`);
}

const bindgenVersion = "0.2.125";
const toolRoot = join(root, "target", `wasm-bindgen-cli-${bindgenVersion}`);
const bindgen = join(toolRoot, "bin", process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen");
const packageOutput = join(output, "pkg");

run("cargo", [
  "build",
  ...(release ? ["--release"] : []),
  "--target", "wasm32-unknown-unknown",
  "-p", "physbox-web",
  ...(!box2dComparator ? ["--no-default-features"] : []),
  ...(solverLab ? ["--features", "solver-lab"] : []),
]);
if (!existsSync(bindgen)) {
  run("cargo", ["install", "wasm-bindgen-cli", "--version", bindgenVersion, "--locked", "--root", toolRoot]);
}
await rm(output, { recursive: true, force: true });
await mkdir(packageOutput, { recursive: true });
await cp(join(root, "web"), output, { recursive: true });
if (box2dComparator) {
  await cp(
    join(root, "third_party", "box2d-comparator", "web"),
    join(output, "box2d-comparator"),
    { recursive: true },
  );
} else {
  const appPath = join(output, "app.js");
  const source = await readFile(appPath, "utf8");
  const withoutImport = source.replace(
    'import init, { box2dComparatorEnabled, PointerButtonCode, start } from "./pkg/physbox_web.js";',
    'import init, { PointerButtonCode, start } from "./pkg/physbox_web.js";',
  );
  const withoutRuntime = withoutImport.replace(
    `  let comparatorRuntime;
  if (box2dComparatorEnabled()) {
    const { loadBox2dComparator } = await import("./box2d-comparator/loader.js");
    comparatorRuntime = await loadBox2dComparator();
  }
`,
    "  const comparatorRuntime = undefined;\n",
  );
  if (
    withoutRuntime === source
    || withoutRuntime.includes("box2dComparator")
    || withoutRuntime.includes("box2d-comparator")
  ) {
    throw new Error("feature-off web glue transformation did not remove comparator loading");
  }
  await writeFile(appPath, withoutRuntime);
}
run(bindgen, [
  join(root, "target", "wasm32-unknown-unknown", release ? "release" : "debug", "physbox_web.wasm"),
  "--target", "web", "--out-dir", packageOutput,
]);
console.log(
  `${release ? "release" : "debug"}${solverLab ? " solver-lab" : ""}${box2dComparator ? " Box2D-comparator" : " comparator-off"} PhysBox 5 web build ready at ${output}`,
);

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, PATH: `${join(toolRoot, "bin")}${delimiter}${process.env.PATH ?? ""}` },
  });
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
}
