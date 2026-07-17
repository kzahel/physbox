#!/usr/bin/env node

import { createHash } from "node:crypto";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

export async function verifySettlingReviewEvidence(packageUrl) {
  const packageDirectory = fileURLToPath(packageUrl);
  const repository = resolve(packageDirectory, "../..");
  const manifest = readJson(resolve(packageDirectory, "manifest.json"));

  assert(manifest.schemaVersion === 1, "unexpected schema version");
  assert(
    manifest.artifactKind === "p5.2b-s6.5-settling-review",
    "unexpected artifact kind",
  );
  assert(manifest.sourceDirty === false, "review package must come from clean source");
  assert(manifest.productDefaultsChanged === false, "product default changed");
  assert(manifest.physicalDefaultsChanged === false, "physical default changed");
  assert(manifest.numericalTolerancesChanged === false, "tolerance changed");
  assert(manifest.oraclePrecedenceChanged === false, "oracle precedence changed");
  assert(manifest.decision.status === "hold-promotion", "promotion hold is missing");
  assert(manifest.decision.promotionAuthorized === false, "promotion was authorized");
  assert(
    manifest.decision.knownFailures.avdCandidate16Collapsed === true,
    "AVD collapse is not retained",
  );
  assert(
    manifest.decision.knownFailures.crossHostStructureGatePassed === false,
    "cross-host failure was hidden",
  );
  assert(
    manifest.decision.candidate.id === "soft-hz30-d10-static1-prior",
    "unexpected candidate",
  );
  assert(manifest.decision.candidate.softness.contactHertz === 30, "unexpected hertz");
  assert(manifest.decision.candidate.softness.dampingRatio === 10, "unexpected damping");
  assert(
    manifest.decision.candidate.softness.staticContactHertzMultiplier === 1,
    "unexpected static multiplier",
  );
  assert(manifest.decision.cost.candidateDispatchDelta === 1, "dispatch delta changed");
  assert(manifest.decision.cost.candidateAllocationDeltaBytes === 0, "allocation changed");
  assert(
    manifest.decision.cost.cpuCrossoverClaimSupported === false,
    "unsupported crossover claim",
  );
  assert(
    manifest.decision.cost.liquidHeadroomClaimSupported === false,
    "unsupported liquid-headroom claim",
  );

  for (const artifact of manifest.inputEvidence) {
    verifyHash(resolve(repository, artifact.path), artifact.sha256, artifact.path);
  }
  for (const artifact of manifest.artifacts) {
    verifyHash(resolve(packageDirectory, artifact.path), artifact.sha256, artifact.path);
  }

  const playback = readJson(resolve(packageDirectory, "playback.json"));
  assert(
    playback.artifactKind === "p5.2b-s6.5-three-way-settling-playback",
    "unexpected playback kind",
  );
  assert(playback.fixture === "centered-stack-8", "unexpected playback fixture");
  assert(playback.sleepingEnabled === false, "sleeping hid awake settling");
  assert(playback.lanes.length === 3, "playback is not three-way");
  assert(
    JSON.stringify(playback.checkpoints) === JSON.stringify([600, 900, 1200]),
    "playback checkpoints changed",
  );
  for (const lane of playback.lanes) {
    assert(lane.samples.length === 3, `lane ${lane.id} sample count changed`);
    assert(
      lane.samples.every((sample, index) => sample.step === playback.checkpoints[index]),
      `lane ${lane.id} is not synchronized`,
    );
  }

  const svg = readFileSync(resolve(packageDirectory, "playback.svg"), "utf8");
  assert(svg.includes("PROMOTION BLOCKER"), "playback omits promotion blocker");
  assert(svg.includes("AVD candidate 16 · collapsed"), "playback omits AVD collapse");

  console.log(
    `PHYSBOX5_VALIDATION_SUCCESS host=evidence scenario=s6.5-settling-review run_id=${manifest.runId}`,
  );
}

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function verifyHash(path, expected, label) {
  const actual = createHash("sha256").update(readFileSync(path)).digest("hex");
  assert(actual === expected, `${label} sha256 ${actual} != ${expected}`);
}

function assert(condition, message) {
  if (!condition) throw new Error(message);
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const packagePath = process.argv[2];
  if (!packagePath) throw new Error("usage: verify-settling-review-evidence.mjs PACKAGE_DIR");
  await verifySettlingReviewEvidence(new URL(`file://${resolve(packagePath)}/`));
}
