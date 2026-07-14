import { cp, mkdir, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import { delimiter, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
let release = false;
let output = join(root, "target", "web");
for (let index = 2; index < process.argv.length; index += 1) {
  const argument = process.argv[index];
  if (argument === "--release") release = true;
  else if (argument === "--output") {
    if (!process.argv[index + 1]) throw new Error("--output requires a path");
    output = resolve(root, process.argv[++index]);
  } else throw new Error(`unknown argument: ${argument}`);
}

const bindgenVersion = "0.2.125";
const toolRoot = join(root, "target", `wasm-bindgen-cli-${bindgenVersion}`);
const bindgen = join(toolRoot, "bin", process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen");
const packageOutput = join(output, "pkg");

run("cargo", ["build", ...(release ? ["--release"] : []), "--target", "wasm32-unknown-unknown", "-p", "physbox-web"]);
if (!existsSync(bindgen)) {
  run("cargo", ["install", "wasm-bindgen-cli", "--version", bindgenVersion, "--locked", "--root", toolRoot]);
}
await rm(output, { recursive: true, force: true });
await mkdir(packageOutput, { recursive: true });
await cp(join(root, "web"), output, { recursive: true });
run(bindgen, [
  join(root, "target", "wasm32-unknown-unknown", release ? "release" : "debug", "physbox_web.wasm"),
  "--target", "web", "--out-dir", packageOutput,
]);
console.log(`${release ? "release" : "debug"} PhysBox 5 web build ready at ${output}`);

function run(command, args) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env: { ...process.env, PATH: `${join(toolRoot, "bin")}${delimiter}${process.env.PATH ?? ""}` },
  });
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
}
