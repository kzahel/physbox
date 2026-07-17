import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve, join } from "node:path";
import { gunzipSync } from "node:zlib";

const root = resolve(process.argv[2] ?? "evidence/p5.2b-s6.2-settling-baseline");
const read = async (path) => readFile(join(root, path));
const json = async (path) => {
  const bytes = await readFile(join(root, path));
  return JSON.parse(path.endsWith(".gz") ? gunzipSync(bytes).toString("utf8") : bytes.toString("utf8"));
};
const digest = (bytes) => createHash("sha256").update(bytes).digest("hex");
const fail = (message) => { throw new Error(message); };
const sha = /^[0-9a-f]{64}$/;

const manifest = await json("manifest.json");
if (manifest.artifactKind !== "p5.2b-s6.2-awake-settling-baseline"
    || manifest.observationVersion !== 2 || manifest.sourceDirty) {
  fail("invalid S6.2 baseline identity or provenance");
}
if (manifest.physicalDefaultsChanged || manifest.numericalTolerancesChanged
    || manifest.oraclePrecedenceChanged || manifest.productConstructorChanged
    || manifest.candidateResultsPresent) {
  fail("S6.2 baseline contains a policy/default/candidate change");
}
if (manifest.box2dRevision !== "3a4f0da8374af61293a03021c9a0b3ebcfe67948"
    || manifest.scenes.length !== 11) {
  fail("S6.2 source authority or canonical scene count changed");
}
const policy = manifest.policy;
if (policy.observationIntervalSteps !== 1 || policy.transientEndStep !== 600
    || policy.settledStartStep !== 900 || policy.settledEndStep !== 1200
    || policy.restLinearSpeedLimit !== 0.025
    || policy.restAngularEdgeSpeedLimit !== 0.025
    || policy.restRequiredConsecutiveObservations !== 60
    || policy.stableSupportRequiredConsecutiveObservations !== 30
    || policy.positiveEnergyRunLimit !== 5) {
  fail("frozen S6.2 measurement policy changed");
}

const verifyArtifact = async (artifact) => {
  const bytes = await read(artifact.path);
  if (digest(bytes) !== artifact.sha256) fail(`artifact digest mismatch: ${artifact.path}`);
};
await verifyArtifact(manifest.playback);

for (const scene of manifest.scenes) {
  for (const value of [scene.sourceScenarioHash, scene.projectionHash,
    scene.gpuRawTraceHash, scene.source.sha256, scene.gpuTrace.sha256]) {
    if (!sha.test(value)) fail(`invalid scene digest: ${scene.label}`);
  }
  if (!scene.gpuReplayExact || !scene.instrumentationFinalExact || !scene.capacityClean
      || !scene.authorityFinalReplayExact || scene.authority.length !== 7) {
    fail(`baseline execution/replay/capacity failed: ${scene.label}`);
  }
  await verifyArtifact(scene.source);
  await verifyArtifact(scene.gpuTrace);
  const trace = await json(scene.gpuTrace.path);
  if (trace.observationVersion !== 2 || trace.sleepingEnabled
      || trace.observations.length !== 1200
      || trace.observations[0].base.completedSteps !== 1
      || trace.observations[1199].base.completedSteps !== 1200
      || JSON.stringify(trace.policy) !== JSON.stringify(policy)
      || JSON.stringify(trace.summary) !== JSON.stringify(scene.summary)) {
    fail(`trace contract mismatch: ${scene.label}`);
  }
  for (const sample of scene.authority) {
    if (!sha.test(sample.requestHash) || !sha.test(sample.resultPayloadHash)) {
      fail(`invalid authority hash: ${scene.label}/${sample.completedSteps}`);
    }
    const request = await json(join(sample.artifact, "oracle-request.json"));
    const result = await json(join(sample.artifact, "oracle-result.json"));
    const stdout = await readFile(join(root, sample.artifact, "oracle-stdout.log"), "utf8");
    if (request.sourceScenarioHash !== sample.requestHash
        || result.sourceScenarioHash !== sample.requestHash
        || result.provenance.sourceRevision !== manifest.box2dRevision
        || result.provenance.sourceDirty || result.failure !== null
        || !stdout.split(/\r?\n/).includes("PHYSBOX5_ORACLE_SUCCESS")) {
      fail(`invalid authority artifact: ${scene.label}/${sample.completedSteps}`);
    }
  }
}

const randomized = manifest.randomized;
if (randomized.seedCount !== 14 || randomized.cases.length !== 14
    || randomized.exactReplays !== 14 || randomized.capacityCleanRuns !== 14) {
  fail("randomized exact-replay/capacity corpus failed");
}
for (const value of randomized.cases) {
  if (!value.replayExact || !value.capacityClean || !sha.test(value.sourceScenarioHash)
      || !sha.test(value.resultHash)) {
    fail(`invalid randomized row: ${value.seed}`);
  }
}
for (const failure of manifest.minimizedFailures) {
  if (!sha.test(failure.sourceScenarioHash) || !sha.test(failure.resultHash)) {
    fail(`invalid minimized failure identity: ${failure.kind}`);
  }
  await verifyArtifact(failure.artifact);
  const artifact = await json(failure.artifact.path);
  if (artifact.sourceScenarioHash !== failure.sourceScenarioHash
      || artifact.resultHash !== failure.resultHash || artifact.attempts !== failure.attempts) {
    fail(`minimized failure artifact mismatch: ${failure.kind}`);
  }
}

console.log(`PHYSBOX5_VALIDATION_SUCCESS host=evidence scenario=s6.2-awake-settling-baseline run_id=${manifest.runId}`);
