import { createHash } from "node:crypto";
import { cp, mkdir, readFile, readdir, rename, rm, stat, writeFile } from "node:fs/promises";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve(import.meta.dirname, "..");
let release = false;
let pages = false;
let output = join(root, "target", "showcase-web");
for (let index = 2; index < process.argv.length; index += 1) {
  const argument = process.argv[index];
  if (argument === "--release") release = true;
  else if (argument === "--pages") pages = true;
  else if (argument === "--output") {
    if (!process.argv[index + 1]) throw new Error("--output requires a path");
    output = resolve(root, process.argv[++index]);
  } else {
    throw new Error(`unknown argument: ${argument}`);
  }
}

const bindgenVersion = "0.2.125";
const toolRoot = join(root, "target", `wasm-bindgen-cli-${bindgenVersion}`);
const bindgen = join(
  toolRoot,
  "bin",
  process.platform === "win32" ? "wasm-bindgen.exe" : "wasm-bindgen",
);
const assets = pages ? join(output, "assets") : output;
const packageOutput = join(assets, "pkg");
const buildEnvironment = { ...process.env };
const remapFlags = [
  `--remap-path-prefix=${root}=/physbox`,
  `--remap-path-prefix=${resolve(process.env.CARGO_HOME ?? join(homedir(), ".cargo"))}=/cargo`,
];
if (buildEnvironment.CARGO_ENCODED_RUSTFLAGS) {
  buildEnvironment.CARGO_ENCODED_RUSTFLAGS = [
    buildEnvironment.CARGO_ENCODED_RUSTFLAGS,
    ...remapFlags,
  ].join("\x1f");
} else {
  buildEnvironment.RUSTFLAGS = [
    buildEnvironment.RUSTFLAGS,
    ...remapFlags,
  ]
    .filter(Boolean)
    .join(" ");
}

run("cargo", [
  "build",
  ...(release ? ["--release"] : []),
  "--target",
  "wasm32-unknown-unknown",
  "-p",
  "physbox-showcase-web",
], buildEnvironment);
if (!existsSync(bindgen)) {
  run("cargo", [
    "install",
    "wasm-bindgen-cli",
    "--version",
    bindgenVersion,
    "--locked",
    "--root",
    toolRoot,
  ]);
}
await rm(output, { recursive: true, force: true });
await mkdir(packageOutput, { recursive: true });
await cp(join(root, "showcase-web"), assets, { recursive: true });
run(bindgen, [
  join(
    root,
    "target",
    "wasm32-unknown-unknown",
    release ? "release" : "debug",
    "physbox_showcase_web.wasm",
  ),
  "--target",
  "web",
  "--out-dir",
  packageOutput,
]);
if (pages) await packagePages();
console.log(`${release ? "release" : "debug"} PhysBox Web Showcase ready at ${output}`);

// Pages caches each URL separately. Relative imports (including bindgen's WASM
// fetch) stay in one content-addressed directory for the lifetime of a page.
async function packagePages() {
  const indexPath = join(assets, "index.html");
  const index = await readFile(indexPath, "utf8");
  await rm(indexPath);
  const digest = createHash("sha256");
  const paths = (await readdir(assets, { recursive: true }))
    .map((path) => path.replaceAll("\\", "/"))
    .sort();
  for (const path of paths) {
    const file = join(assets, path);
    if (!(await stat(file)).isFile()) continue;
    const bytes = await readFile(file);
    digest.update(`${path}\0${bytes.length}\0`);
    digest.update(bytes);
  }
  const bundle = `assets-${digest.digest("hex").slice(0, 24)}`;
  await rename(assets, join(output, bundle));
  await writeFile(join(output, "index.html"), index
    .replace('href="./styles.css"', `href="./${bundle}/styles.css"`)
    .replace('src="./app.js"', `src="./${bundle}/app.js"`));
}

function run(command, args, environment = process.env) {
  const result = spawnSync(command, args, {
    cwd: root,
    stdio: "inherit",
    env: {
      ...environment,
      PATH: `${join(toolRoot, "bin")}${delimiter}${environment.PATH ?? ""}`,
    },
  });
  if (result.status !== 0) {
    throw new Error(`${command} failed with status ${result.status ?? "unknown"}`);
  }
}
