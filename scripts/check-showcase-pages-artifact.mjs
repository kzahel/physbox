import { createHash } from "node:crypto";
import { lstat, readFile, readdir } from "node:fs/promises";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const directory = resolve(root, process.argv[2] ?? "dist-showcase");
const index = await readFile(resolve(directory, "index.html"), "utf8");
const bundle = index.match(/src="\.\/(assets-[a-f0-9]{24})\/app\.js"/u)?.[1];
if (!bundle) throw new Error("Pages entry must load a versioned showcase bundle");
const required = [
  "app.js",
  "pkg/physbox_showcase_web.js",
  "pkg/physbox_showcase_web_bg.wasm",
  "startup-diagnostics.js",
  "styles.css",
];
const forbiddenSourceMachineValues = [
  Buffer.from("/home/"),
  Buffer.from("/Users/"),
  Buffer.from("/private/"),
  Buffer.from("/tmp/"),
  Buffer.from("C:\\Users\\"),
  Buffer.from(".ts.net"),
];
const names = new Set();

await walk(directory);
for (const name of required) {
  if (!names.has(`${bundle}/${name}`)) {
    throw new Error(`Pages artifact is missing ${bundle}/${name}`);
  }
}
if (names.has("_headers") || names.has("CNAME")) {
  throw new Error("Pages artifact unexpectedly configures headers or a custom domain");
}

const digest = createHash("sha256");
const bundleDigest = createHash("sha256");
let bytes = 0;
for (const name of [...names].sort()) {
  const path = resolve(directory, name);
  const content = await readFile(path);
  const size = content.length;
  bytes += size;
  if (name.startsWith(`${bundle}/`)) {
    bundleDigest.update(`${name.slice(bundle.length + 1)}\0${size}\0`);
    bundleDigest.update(content);
  }
  digest.update(`${name}\0${size}\0`);
  digest.update(content);
  for (const forbidden of forbiddenSourceMachineValues) {
    if (content.includes(forbidden)) {
      throw new Error(
        `${name} contains source-machine value ${forbidden.toString("utf8")}`,
      );
    }
  }
}

if (bundle !== `assets-${bundleDigest.digest("hex").slice(0, 24)}`) {
  throw new Error("Pages bundle URL does not match its contents");
}

for (const expected of [
  `href="./${bundle}/styles.css"`,
  "https://github.com/kzahel/physbox",
]) {
  if (!index.includes(expected)) {
    throw new Error(`index.html is missing ${expected}`);
  }
}
if (/(?:href|src)="\/(?!\/)/u.test(index)) {
  throw new Error("index.html contains a root-relative asset URL");
}

console.log(
  `PHYSBOX_SHOWCASE_ARTIFACT files=${names.size} bytes=${bytes} sha256=${digest.digest("hex")}`,
);

async function walk(path) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, entry.name);
    const metadata = await lstat(child);
    if (metadata.isSymbolicLink()) {
      throw new Error(`Pages artifact contains symlink ${relative(directory, child)}`);
    }
    if (metadata.isDirectory()) await walk(child);
    else if (metadata.isFile()) names.add(relative(directory, child).replaceAll("\\", "/"));
    else {
      throw new Error(
        `Pages artifact contains unsupported entry ${relative(directory, child)}`,
      );
    }
  }
}
