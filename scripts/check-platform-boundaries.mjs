import { readFile, readdir } from "node:fs/promises";
import { execFile } from "node:child_process";
import { extname, join, resolve, relative } from "node:path";
import { promisify } from "node:util";

const root = resolve(import.meta.dirname, "..");
const appRoot = join(root, "apps");
const webRoot = join(root, "web");
const failures = [];
const execFileAsync = promisify(execFile);
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
const evidencePathPattern = /^evidence(?:\/|$)/;
const authoredWebExtensions = new Set([".js", ".mjs", ".ts", ".tsx"]);
const productWebPath = "web/app.js";
const observerWebPath = "web/smoke-observer.js";
const maxProductWebLines = 120;
const maxObserverWebLines = 40;
const productWebSemanticRules = [
  {
    name: "tool, menu, grab, placement, or construction semantic",
    pattern: /\b(?:currentTool|menuOpen|WorkbenchTool|GrabBegin|GrabTarget|GrabEnd|grabPointerActive|ConstructionIntent|PlaceCircle|PlaceBox|PlacePolygon|placement|construction)\b/i,
  },
  {
    name: "physics configuration or preference policy",
    pattern: /\b(?:solverProfile|fixedStepHz|solverSubsteps|solverBiasedIterations|requestedContactHz|requestedStaticContactHz|requestedSleepingEnabled|bodyCapacity|physicsPreference|worldPreference)\b|physbox5\.(?:physics|world|solver)|["'](?:soft-coupled|hard-direct|body-capacity|solver-options|sim-hz|substeps|iterations|contact-hz|contact-damping|static-contact-scale|speculative-distance|max-depenetration-velocity|sleeping)["']/i,
  },
  {
    name: "semantic engine report access",
    pattern: /\b(?:sceneContract|bodyCommands|scenarioHash|markerJson|observerSnapshot|diagnosticsJson)\b/,
  },
  {
    name: "product-global diagnostic publication",
    pattern: /\bglobalThis\b|__physbox5/,
  },
  {
    name: "browser-owned persistence schema",
    pattern: /\blocalStorage\b/,
  },
];
const requiredProductCalls = [
  "observerEnabled",
  "pointerEvent",
  "render",
  "resize",
  "wheelEvent",
];

const { stdout: trackedEvidence } = await execFileAsync("git", ["ls-files", "--", "evidence"], { cwd: root });
for (const path of trackedEvidence.split("\n").filter(Boolean)) {
  if (evidencePathPattern.test(path)) failures.push(`${path}: generated evidence must remain outside Git`);
}

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

const authoredWebPaths = (await files(webRoot))
  .filter(isAuthoredWebSource)
  .filter((path) => !relative(webRoot, path).startsWith(`pkg${process.platform === "win32" ? "\\" : "/"}`));
const relativeAuthoredWebPaths = authoredWebPaths.map((path) => relative(root, path));
failures.push(...productWebInventoryFailures(relativeAuthoredWebPaths));

for (const path of authoredWebPaths) {
  const source = await readFile(path, "utf8");
  const repoPath = relative(root, path);
  if (scenarioPayloadPattern.test(source)) {
    failures.push(`${repoPath}: engine-owned scenario payload`);
  }
  if (repoPath === productWebPath) {
    failures.push(...productWebSourceFailures(repoPath, source));
  } else if (repoPath === observerWebPath) {
    failures.push(...observerWebSourceFailures(repoPath, source));
  }
}

for (const path of await files(join(root, "android"))) {
  if (![".js", ".mjs", ".ts", ".tsx", ".sh"].includes(extname(path))) continue;
  if (scenarioPayloadPattern.test(await readFile(path, "utf8"))) {
    failures.push(`${relative(root, path)}: engine-owned scenario payload`);
  }
}

const webHtml = await readFile(join(webRoot, "index.html"), "utf8");
if ((webHtml.match(/<canvas\b/g) ?? []).length !== 1 || (webHtml.match(/<p\b/g) ?? []).length !== 1) {
  failures.push("web/index.html: browser body must remain one canvas plus one startup-fatal paragraph");
}
if (/<(?:button|nav|aside|header|footer|section|form)\b/i.test(webHtml)) {
  failures.push("web/index.html: product DOM chrome belongs to the shared renderer");
}
if (webHtml.includes("smoke-observer")) {
  failures.push("web/index.html: smoke observer must not load during ordinary product operation");
}

const webAppRust = await readFile(join(appRoot, "web", "src", "lib.rs"), "utf8");
const webAppImpl = sourceSection(
  webAppRust,
  "impl WebApp {",
  "#[cfg(target_arch = \"wasm32\")]\n#[wasm_bindgen]\npub async fn start",
);
if (!webAppImpl) {
  failures.push("apps/web/src/lib.rs: WebApp boundary section was not found");
} else if (/\b(?:WorkbenchTool|GrabIntent|ConstructionIntent|UiAction|scene_contract|body_commands|current_tool|menu_open|place_circle|place_box|place_polygon)\b/i.test(webAppImpl)) {
  failures.push("apps/web/src/lib.rs: WebApp bridge contains engine-owned interaction semantics");
}

const workspace = await readFile(join(root, "Cargo.toml"), "utf8");
if (/\brapier\b|physbox-rigid/i.test(workspace)) failures.push("Cargo.toml: inherited CPU rigid backend is forbidden");

const uiManifestPath = join(root, "crates", "physbox-ui", "Cargo.toml");
const uiManifest = await readFile(uiManifestPath, "utf8");
const uiForbiddenDependencies = /\bphysbox-(?:engine|launch|contracts|box2d-comparator|shell)\s*=/i;
if (uiForbiddenDependencies.test(uiManifest)) {
  failures.push("crates/physbox-ui/Cargo.toml: neutral UI crate depends on product policy");
}
const uiProductVocabulary = /\b(?:SolverProfile|ValidationScenario|WorkbenchTool|ComparisonEngine|SleepGrouping|UiAction)\b/;
for (const path of await files(join(root, "crates", "physbox-ui", "src"))) {
  if (extname(path) === ".rs" && uiProductVocabulary.test(await readFile(path, "utf8"))) {
    failures.push(`${relative(root, path)}: product vocabulary belongs to physbox-engine`);
  }
}

const simManifestPath = join(root, "crates", "physbox-sim", "Cargo.toml");
const simManifest = await readFile(simManifestPath, "utf8");
const simForbiddenDependencies = /\bphysbox-(?:engine|launch|ui|box2d-comparator|shell)\s*=/i;
if (simForbiddenDependencies.test(simManifest)) {
  failures.push("crates/physbox-sim/Cargo.toml: simulation library depends on application policy");
}
const simProductVocabulary = /\b(?:ValidationScenario|WorkbenchTool|UiAction|LaunchOptions|RenderScalePreset|Box2dWorld|ComparisonShadow)\b/;
for (const path of await files(join(root, "crates", "physbox-sim", "src"))) {
  if (extname(path) === ".rs" && simProductVocabulary.test(await readFile(path, "utf8"))) {
    failures.push(`${relative(root, path)}: application vocabulary belongs outside physbox-sim`);
  }
}

const renderManifestPath = join(root, "crates", "physbox-render", "Cargo.toml");
const renderManifest = await readFile(renderManifestPath, "utf8");
const renderForbiddenDependencies = /\bphysbox-(?:engine|launch|ui|box2d-comparator|shell)\s*=/i;
if (renderForbiddenDependencies.test(renderManifest)) {
  failures.push("crates/physbox-render/Cargo.toml: renderer library depends on application policy");
}
const renderProductVocabulary = /\b(?:ValidationScenario|WorkbenchTool|UiAction|LaunchOptions|Box2dWorld|ComparisonShadow)\b/;
for (const path of await files(join(root, "crates", "physbox-render", "src"))) {
  if (extname(path) === ".rs" && renderProductVocabulary.test(await readFile(path, "utf8"))) {
    failures.push(`${relative(root, path)}: application vocabulary belongs outside physbox-render`);
  }
}

const consumerManifestPath = join(root, "examples", "external-consumer", "Cargo.toml");
const consumerManifest = await readFile(consumerManifestPath, "utf8");
const consumerForbiddenDependencies = /\bphysbox-(?:engine|launch|ui|box2d-comparator|shell|contracts)\s*=/i;
if (consumerForbiddenDependencies.test(consumerManifest)) {
  failures.push("examples/external-consumer/Cargo.toml: consumer proof imports application policy");
}
const consumerSource = await readFile(
  join(root, "examples", "external-consumer", "src", "main.rs"),
  "utf8",
);
if (/\b(?:ValidationScenario|WorkbenchTool|LaunchOptions|ScenarioV1|Box2dWorld|SurfaceRenderer)\b/.test(consumerSource)) {
  failures.push("examples/external-consumer/src/main.rs: consumer proof uses a non-library application type");
}

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
if (!uiForbiddenDependencies.test("physbox-engine = { path = \"../physbox-engine\" }")) {
  failures.push("boundary self-test failed: UI dependency direction");
}
if (!uiProductVocabulary.test("enum WorkbenchTool { Grab }")) {
  failures.push("boundary self-test failed: UI product vocabulary");
}
if (!simForbiddenDependencies.test("physbox-engine = { path = \"../physbox-engine\" }")) {
  failures.push("boundary self-test failed: simulation dependency direction");
}
if (!simProductVocabulary.test("struct LaunchOptions;")) {
  failures.push("boundary self-test failed: simulation application vocabulary");
}
if (!renderForbiddenDependencies.test("physbox-engine = { path = \"../physbox-engine\" }")) {
  failures.push("boundary self-test failed: renderer dependency direction");
}
if (!renderProductVocabulary.test("enum WorkbenchTool { Grab }")) {
  failures.push("boundary self-test failed: renderer application vocabulary");
}
if (!consumerForbiddenDependencies.test("physbox-engine = { path = \"../../crates/physbox-engine\" }")) {
  failures.push("boundary self-test failed: external consumer dependency");
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
if (!evidencePathPattern.test("evidence/accidental-force-add/manifest.json")) {
  failures.push("boundary self-test failed: tracked evidence path");
}
if (!isAuthoredWebSource("web/future-glue.ts") || !isAuthoredWebSource("web/future-glue.tsx")) {
  failures.push("boundary self-test failed: TypeScript source discovery");
}
if (productWebInventoryFailures([productWebPath, observerWebPath, "web/future-glue.ts"]).length === 0) {
  failures.push("boundary self-test failed: added TypeScript product-glue inventory");
}
if (!productWebSourceFailures("web/future-glue.ts", 'const currentTool = "grab";')
  .some((failure) => failure.includes("tool, menu, grab"))) {
  failures.push("boundary self-test failed: TypeScript interaction semantics");
}
if (!productWebSourceFailures(productWebPath, "app.selectTool();")
  .some((failure) => failure.includes("WASM calls must be exactly"))) {
  failures.push("boundary self-test failed: expanded WASM call surface");
}
if (!observerWebSourceFailures(observerWebPath, "app.pointerEvent(1, 0, 0, 0, 0);")
  .some((failure) => failure.includes("may not drive product"))) {
  failures.push("boundary self-test failed: observer product mutation");
}

if (failures.length) {
  console.error(failures.join("\n"));
  process.exit(1);
}
console.log("PhysBox 5 platform boundaries passed (including JS/TS discovery and negative fixtures)");

function productWebInventoryFailures(paths) {
  const productPaths = paths.filter((path) => path !== observerWebPath);
  if (
    productPaths.length === 1
    && productPaths[0] === productWebPath
    && paths.includes(observerWebPath)
  ) {
    return [];
  }
  return [
    `web: authored product glue must remain ${productWebPath} plus the sole ${observerWebPath} test exception; found ${paths.join(", ")}`,
  ];
}

function productWebSourceFailures(path, source) {
  const sourceFailures = [];
  for (const rule of productWebSemanticRules) {
    if (rule.pattern.test(source)) sourceFailures.push(`${path}: ${rule.name}`);
  }
  const lines = lineCount(source);
  if (lines > maxProductWebLines) {
    sourceFailures.push(`${path}: authored product glue is ${lines} lines; maximum is ${maxProductWebLines}`);
  }
  if (!/import init, \{ box2dComparatorEnabled, PointerButtonCode, start \} from "\.\/pkg\/physbox_web\.js";/.test(source)) {
    sourceFailures.push(`${path}: WASM import surface changed`);
  }
  const calls = [...source.matchAll(/\bapp\.([A-Za-z_$][\w$]*)\s*\(/g)]
    .map((match) => match[1]);
  const uniqueCalls = [...new Set(calls)].sort();
  if (uniqueCalls.join(",") !== requiredProductCalls.join(",")) {
    sourceFailures.push(`${path}: WASM calls must be exactly ${requiredProductCalls.join(", ")}; found ${uniqueCalls.join(", ")}`);
  }
  if (
    !source.includes("if (app.observerEnabled())")
    || !source.includes('await import("./smoke-observer.js")')
  ) {
    sourceFailures.push(`${path}: smoke observer must remain dynamically and explicitly gated`);
  }
  return sourceFailures;
}

function observerWebSourceFailures(path, source) {
  const sourceFailures = [];
  const lines = lineCount(source);
  if (lines > maxObserverWebLines) {
    sourceFailures.push(`${path}: smoke observer is ${lines} lines; maximum is ${maxObserverWebLines}`);
  }
  for (const required of ["app.diagnosticsJson()", "app.observerSnapshot()", "__physbox5", "__physbox5FrameControl"]) {
    if (!source.includes(required)) sourceFailures.push(`${path}: missing observer surface ${required}`);
  }
  if (/\bapp\.(?:pointerEvent|wheelEvent|resize|render)\s*\(/.test(source)) {
    sourceFailures.push(`${path}: smoke observer may not drive product input, resize, or rendering`);
  }
  if (/\b(?:localStorage|sessionStorage|history|document|canvas|addEventListener)\b/.test(source)) {
    sourceFailures.push(`${path}: smoke observer may not own product storage, DOM, or event handling`);
  }
  return sourceFailures;
}

function sourceSection(source, startMarker, endMarker) {
  const start = source.indexOf(startMarker);
  if (start < 0) return "";
  const end = source.indexOf(endMarker, start);
  return end < 0 ? "" : source.slice(start, end);
}

function isAuthoredWebSource(path) {
  return authoredWebExtensions.has(extname(path));
}

function lineCount(source) {
  return source.replace(/\n$/, "").split(/\r?\n/).length;
}

async function files(directory) {
  const output = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) output.push(...await files(path));
    else output.push(path);
  }
  return output;
}
