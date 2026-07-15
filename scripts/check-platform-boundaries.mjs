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
    pattern: /\b(?:RigidBody|ContactManifold|Broadphase|Narrowphase|PhysicsSolver|ParticleSolver|ColliderRecord|BodyCommandV1|GrabBegin|GrabTarget|GrabEnd)\b/,
  },
  {
    name: "Rapier or inherited rigid backend",
    pattern: /\brapier\b|physbox-rigid/i,
  },
];
const scenarioPayloadPattern = /\b(?:worldTarget|linearVelocity|angularVelocity|halfExtents|normalImpulse|biasRate|massCoefficient|impulseCoefficient)\b/;

for (const path of await files(appRoot)) {
  if (![".rs", ".toml"].includes(extname(path))) continue;
  const source = await readFile(path, "utf8");
  for (const rule of rules) {
    if (rule.pattern.test(source)) failures.push(`${relative(root, path)}: ${rule.name}`);
  }
  for (const flag of ["--width", "--height", "--frames", "--fps", "--steps", "--output", "--report", "--scenario"]) {
    if (source.includes(`"${flag}"`)) failures.push(`${relative(root, path)}: launch option ${flag} belongs to physbox-launch`);
  }
}

for (const directory of [join(root, "web"), join(root, "android")]) {
  for (const path of await files(directory)) {
    if (![".js", ".sh"].includes(extname(path))) continue;
    if (scenarioPayloadPattern.test(await readFile(path, "utf8"))) {
      failures.push(`${relative(root, path)}: engine-owned scenario payload`);
    }
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

const oracleDependency = /physbox-box2d-oracle\s*=/i;
for (const rootName of ["apps", "crates"]) {
  for (const path of await files(join(root, rootName))) {
    if (path.endsWith("Cargo.toml") && oracleDependency.test(await readFile(path, "utf8"))) {
      failures.push(`${relative(root, path)}: native Box2D oracle dependency is forbidden outside test tooling`);
    }
  }
}
if (!oracleDependency.test('physbox-box2d-oracle = { path = "../../tools/oracles/box2d" }')) {
  failures.push("boundary self-test failed: native Box2D oracle dependency");
}

for (const rule of rules) {
  const negativeFixture = rule.name.includes("WGSL")
    ? "app.create_compute_pipeline();"
    : rule.name.includes("physics")
      ? "struct PhysicsSolver;"
      : "rapier = \"*\"";
  if (!rule.pattern.test(negativeFixture)) failures.push(`boundary self-test failed: ${rule.name}`);
}
if (!scenarioPayloadPattern.test("const worldTarget = [0.4, 0.7];")) {
  failures.push("boundary self-test failed: engine-owned scenario payload");
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
