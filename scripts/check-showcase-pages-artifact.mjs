import { createHash } from "node:crypto";
import { lstat, readFile, readdir } from "node:fs/promises";
import { relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "..");
const directory = resolve(root, process.argv[2] ?? "dist-showcase");
const required = [
  "app.js",
  "index.html",
  "pkg/physbox_showcase_web.js",
  "pkg/physbox_showcase_web_bg.wasm",
  "startup-diagnostics.js",
  "styles.css",
];
const textExtensions = new Set([".css", ".d.ts", ".html", ".js"]);
const files = [];

await walk(directory);
const names = new Set(files.map(({ path }) => relative(directory, path)));
for (const name of required) {
  if (!names.has(name)) throw new Error(`Pages artifact is missing ${name}`);
}
if (names.has("_headers") || names.has("CNAME")) {
  throw new Error("Pages artifact unexpectedly configures headers or a custom domain");
}

const digest = createHash("sha256");
let bytes = 0;
for (const { path, size } of files.sort((left, right) =>
  left.path.localeCompare(right.path),
)) {
  const name = relative(directory, path);
  const content = await readFile(path);
  bytes += size;
  digest.update(`${name}\0${size}\0`);
  digest.update(content);
  if (textExtensions.has(extension(name))) {
    const text = content.toString("utf8");
    for (const forbidden of [
      "/home/",
      "C:\\Users\\",
      ".ts.net",
    ]) {
      if (text.includes(forbidden)) {
        throw new Error(`${name} contains source-machine value ${forbidden}`);
      }
    }
  }
}

const index = await readFile(resolve(directory, "index.html"), "utf8");
for (const expected of [
  'href="./styles.css"',
  'src="./app.js"',
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
  `PHYSBOX_SHOWCASE_ARTIFACT files=${files.length} bytes=${bytes} sha256=${digest.digest("hex")}`,
);

async function walk(path) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, entry.name);
    const metadata = await lstat(child);
    if (metadata.isSymbolicLink()) {
      throw new Error(`Pages artifact contains symlink ${relative(directory, child)}`);
    }
    if (metadata.isDirectory()) await walk(child);
    else if (metadata.isFile()) files.push({ path: child, size: metadata.size });
    else {
      throw new Error(
        `Pages artifact contains unsupported entry ${relative(directory, child)}`,
      );
    }
  }
}

function extension(path) {
  if (path.endsWith(".d.ts")) return ".d.ts";
  const index = path.lastIndexOf(".");
  return index < 0 ? "" : path.slice(index);
}
