import { readFile, readdir } from "node:fs/promises";
import { extname, join, resolve, relative } from "node:path";

const root = resolve(import.meta.dirname, "..");
const appRoot = join(root, "apps");
const failures = [];
const rules = [
  {
    name: "app-local WGSL or GPU pass encoding",
    pattern: /\.wgsl\b|create_shader_module|create_(?:compute|render)_pipeline|begin_(?:compute|render)_pass/i,
  },
  {
    name: "app-local physics/product type",
    pattern: /\b(?:RigidBody|ContactManifold|Broadphase|Narrowphase|PhysicsSolver|ParticleSolver|ColliderRecord)\b/,
  },
  {
    name: "Rapier or inherited rigid backend",
    pattern: /\brapier\b|physbox-rigid/i,
  },
];

for (const path of await files(appRoot)) {
  if (![".rs", ".toml"].includes(extname(path))) continue;
  const source = await readFile(path, "utf8");
  for (const rule of rules) {
    if (rule.pattern.test(source)) failures.push(`${relative(root, path)}: ${rule.name}`);
  }
  for (const flag of ["--width", "--height", "--frames", "--fps", "--output", "--report", "--scenario"]) {
    if (source.includes(`"${flag}"`)) failures.push(`${relative(root, path)}: launch option ${flag} belongs to physbox-launch`);
  }
}

const webHtml = await readFile(join(root, "web", "index.html"), "utf8");
if ((webHtml.match(/<canvas\b/g) ?? []).length !== 1 || (webHtml.match(/<p\b/g) ?? []).length !== 1) {
  failures.push("web/index.html: browser body must remain one canvas plus one startup-fatal paragraph");
}
if (/<(?:button|nav|aside|header|footer|section|form)\b/i.test(webHtml)) {
  failures.push("web/index.html: product DOM chrome belongs to the shared renderer");
}

const workspace = await readFile(join(root, "Cargo.toml"), "utf8");
if (/\brapier\b|physbox-rigid/i.test(workspace)) failures.push("Cargo.toml: inherited CPU rigid backend is forbidden");

for (const rule of rules) {
  const negativeFixture = rule.name.includes("WGSL")
    ? "app.create_compute_pipeline();"
    : rule.name.includes("physics")
      ? "struct PhysicsSolver;"
      : "rapier = \"*\"";
  if (!rule.pattern.test(negativeFixture)) failures.push(`boundary self-test failed: ${rule.name}`);
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log("PhysBox 5 platform boundaries passed (including negative fixtures)");

async function files(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...await files(path));
    else output.push(path);
  }
  return output;
}
