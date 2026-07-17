import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";

const root = resolve(process.argv[2] ?? "evidence/p5.2b-s6.3-contact-identity");
const manifest = JSON.parse(await readFile(resolve(root, "manifest.json"), "utf8"));

const fail = (message) => {
  throw new Error(`S6.3 contact-identity evidence: ${message}`);
};
const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");
const verifyArtifact = async (artifact) => {
  if (!artifact || typeof artifact.path !== "string" || !/^[0-9a-f]{64}$/.test(artifact.sha256)) {
    fail("invalid artifact identity");
  }
  const path = artifact.path.startsWith("evidence/")
    ? resolve(artifact.path)
    : resolve(root, artifact.path);
  const bytes = await readFile(path);
  if (sha256(bytes) !== artifact.sha256) fail(`digest mismatch for ${artifact.path}`);
};

if (
  manifest.schemaVersion !== 1 ||
  manifest.observationVersion !== 2 ||
  manifest.artifactKind !== "p5.2b-s6.3-contact-identity" ||
  manifest.sourceDirty !== false ||
  manifest.selectedPolicy !== "direct-maximum-separation" ||
  manifest.physicalDefaultsChanged !== false ||
  manifest.numericalTolerancesChanged !== false ||
  manifest.oraclePrecedenceChanged !== false ||
  manifest.productConstructorChanged !== false
) {
  fail("manifest identity/provenance/default declarations are invalid");
}
await verifyArtifact(manifest.baselineManifest);
await verifyArtifact(manifest.playback);

const policies = [
  "direct-maximum-separation",
  "relative-coherence-bias",
  "canonical-absolute-band",
  "prior-contact-hysteresis",
];
if (manifest.policies.length !== 4 || manifest.scenes.length !== 11) {
  fail("incomplete policy or scene matrix");
}
for (const [index, policy] of manifest.policies.entries()) {
  if (policy.policy !== policies[index] || policy.sceneCount !== 11) {
    fail("policy summary ordering/count is invalid");
  }
}
const direct = manifest.policies[0];
const relative = manifest.policies[1];
const canonical = manifest.policies[2];
const temporal = manifest.policies[3];
if (
  direct.settledOwnerChanges !== 3068 ||
  direct.settledFeatureChanges !== 3068 ||
  direct.settledCacheMisses !== 6136 ||
  relative.hardGatePasses !== 7 ||
  canonical.hardGatePasses !== 7 ||
  temporal.hardGatePasses !== 7 ||
  temporal.settledOwnerChanges !== 0 ||
  temporal.settledFeatureChanges !== 0 ||
  temporal.settledCacheMisses !== 0 ||
  temporal.settledPointCountChanges !== 0 ||
  temporal.settledNormalChanges !== 0
) {
  fail("contact identity conclusion matrix changed");
}
for (const scene of manifest.scenes) {
  await verifyArtifact(scene.source);
  if (scene.candidates.length !== 4) fail(`${scene.label} omitted a candidate`);
  for (const [index, candidate] of scene.candidates.entries()) {
    if (
      candidate.policy !== policies[index] ||
      candidate.replayExact !== true ||
      candidate.instrumentationFinalExact !== true ||
      candidate.capacityClean !== true
    ) {
      fail(`${scene.label}/${policies[index]} failed replay/capacity/structure`);
    }
    await verifyArtifact(candidate.trace);
  }
}
if (manifest.randomized.length !== 4 || manifest.timing.length !== 4) {
  fail("randomized or timing matrix is incomplete");
}
for (const [index, random] of manifest.randomized.entries()) {
  if (
    random.policy !== policies[index] ||
    random.seedCount !== 14 ||
    random.exactReplays !== 14 ||
    random.capacityCleanRuns !== 14 ||
    random.structuralPasses !== 14
  ) {
    fail(`randomized ${policies[index]} qualification failed`);
  }
}
for (const [index, timing] of manifest.timing.entries()) {
  if (
    timing.policy !== policies[index] ||
    timing.bodyCount !== 256 ||
    timing.samples < 1 ||
    timing.dispatchesPerStep < 1 ||
    timing.allocatedBytes < 1 ||
    !/^[0-9a-f]{64}$/.test(timing.workDigest)
  ) {
    fail(`timing ${policies[index]} is invalid`);
  }
  await verifyArtifact(timing.raw);
}
if (
  manifest.timing[3].dispatchesPerStep !== manifest.timing[0].dispatchesPerStep + 1 ||
  manifest.timing[3].allocatedBytes !== manifest.timing[0].allocatedBytes
) {
  fail("temporal identity cost declaration changed");
}
const stack16 = manifest.scenes.find((scene) => scene.label === "centered-stack-16");
if (
  !stack16 ||
  stack16.candidates[0].structuralPass !== true ||
  stack16.candidates[1].structuralPass !== false ||
  stack16.candidates[2].structuralPass !== false ||
  stack16.candidates[3].structuralPass !== false ||
  stack16.candidates[3].summary.settledLinearSpeedMax >= 0.05
) {
  fail("deep-stack rejection/temporal diagnostic changed");
}
if (
  manifest.comparatorDecisions.truePriorContactHysteresis !==
    "forward-for-s6.4-combination" ||
  manifest.comparatorDecisions.relativeCoherenceBias !==
    "rejected-deep-stack-instability" ||
  manifest.comparatorDecisions.canonicalAbsoluteBand !==
    "rejected-deep-stack-instability" ||
  manifest.comparatorDecisions.canonicalImpulseCorrespondence !==
    "rejected-without-safe-geometry-proof"
) {
  fail("stateful comparator decision is missing");
}

console.log(
  `PHYSBOX5_VALIDATION_SUCCESS host=evidence scenario=s6.3-contact-identity run_id=${manifest.runId}`,
);
