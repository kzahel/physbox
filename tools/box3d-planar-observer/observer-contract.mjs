import { readFileSync } from "node:fs";

export const box3dPin = "8441b4a06d6d09dcfb0b0f704df4d847d1437b92";

function assert(condition, message) {
  if (!condition) {
    throw new Error(message);
  }
}

function assertFinite(value, path = "root") {
  if (typeof value === "number") {
    assert(Number.isFinite(value), `${path} is not finite`);
    return;
  }
  if (Array.isArray(value)) {
    value.forEach((entry, index) => assertFinite(entry, `${path}[${index}]`));
    return;
  }
  if (value && typeof value === "object") {
    for (const [key, entry] of Object.entries(value)) {
      assertFinite(entry, `${path}.${key}`);
    }
  }
}

export function verifyObservation(document, expected = {}) {
  assert(document.schema === "physbox5.box3d-planar-observer/v1", "observer schema differs");
  assert(document.source?.engine === "Box3D", "source engine differs");
  assert(document.source?.revision === box3dPin, "Box3D pin differs");
  assert(document.source?.role === "source-reference", "Box3D role differs");

  const contract = document.contract;
  assert(contract && typeof contract === "object", "contract is missing");
  assert(["pyramid2d-12", "large-pyramid-90"].includes(contract.scenario), "scenario differs");
  assert(contract.steps >= 0 && Number.isInteger(contract.steps), "step count is invalid");
  assert(contract.dt === Math.fround(1 / 60), "fixed dt differs");
  assert(contract.substeps === 4, "substep count differs");
  assert(contract.workers === 1, "worker count differs");
  assert(contract.sleeping === false, "sleeping must be disabled");
  assert(contract.warmStarting === true, "warm starting must be enabled");
  assert(contract.contactRecycling === true, "stock contact recycling must be enabled");
  assert(contract.simd === true, "SIMD build contract differs");
  assert(contract.validation === true, "validation build contract differs");
  assert(contract.settledStartStep === 900, "settled window differs");
  assert(contract.nearRestSpeed === 0.05, "near-rest threshold differs");
  assert(contract.toppleAngleRadians === 0.7853981633974483, "topple angle differs");
  assert(contract.minimumHeightRetention === 0.7, "height-retention threshold differs");
  assert(contract.maximumFootprintExpansion === 1.5, "footprint threshold differs");
  assert(JSON.stringify(contract.checkpoints) === JSON.stringify([0, 200, 1200, 5000]), "checkpoints differ");
  const expectedBodies = contract.scenario === "pyramid2d-12" ? 78 : 4095;
  const expectedWidth = contract.scenario === "pyramid2d-12" ? 2 : 1;
  assert(contract.dynamicBodies === expectedBodies, "dynamic-body source arithmetic differs");
  assert(contract.displacementWidth === expectedWidth, "displacement threshold differs");
  assert(contract.maximumOutOfPlaneCenters === expectedWidth, "out-of-plane threshold differs");
  assert(contract.planarLocked === (contract.scenario === "pyramid2d-12"), "motion-lock contract differs");
  if (expected.scenario !== undefined) {
    assert(contract.scenario === expected.scenario, "unexpected scenario");
  }
  if (expected.steps !== undefined) {
    assert(contract.steps === expected.steps, "unexpected step count");
  }

  assert(Array.isArray(document.observations) && document.observations.length >= 1, "observations are missing");
  let previousStep = -1;
  for (const observation of document.observations) {
    assert(Number.isInteger(observation.step) && observation.step > previousStep, "observation steps are not strictly ordered");
    previousStep = observation.step;
    const { body, contact, counters } = observation;
    assert(body && contact && counters, `step ${observation.step} is incomplete`);
    assert(body.toppled >= 0 && body.toppled <= expectedBodies, `step ${observation.step} toppled count is invalid`);
    assert(body.displaced >= 0 && body.displaced <= expectedBodies, `step ${observation.step} displaced count is invalid`);
    assert(body.lost >= 0 && body.lost <= expectedBodies, `step ${observation.step} lost count is invalid`);
    if (contract.planarLocked) {
      assert(body.maxAbsZ === 0, `step ${observation.step} violated the linear-Z lock`);
    }
    assert(Array.isArray(body.bounds) && body.bounds.length === 6, `step ${observation.step} body bounds differ`);
    const expectedStructure = body.lost === 0 && body.heightRetention >= contract.minimumHeightRetention &&
      body.footprintExpansion <= contract.maximumFootprintExpansion &&
      (contract.planarLocked || body.maxAbsZ <= contract.maximumOutOfPlaneCenters);
    assert(body.uprightStructure === expectedStructure, `step ${observation.step} structure label differs`);
    assert(contact.contacts >= 0 && contact.manifolds >= 0 && contact.points >= 0, `step ${observation.step} contact counts are invalid`);
    assert(contact.persistedPoints + contact.freshPoints === contact.points, `step ${observation.step} point persistence is incomplete`);
    assert(contact.points <= contact.manifolds * 4, `step ${observation.step} exceeds Box3D's four-point manifold capacity`);
    assert(counters.bodies === expectedBodies + 1, `step ${observation.step} world body count differs`);
    assert(counters.shapes === expectedBodies + 1, `step ${observation.step} world shape count differs`);
    assert(Array.isArray(counters.colors) && counters.colors.length === 24, `step ${observation.step} color counters differ`);
    const checkpoint = observation.step === 0 || observation.step === 200 || observation.step === 1200 ||
      observation.step === 5000 || observation.step === contract.steps;
    if (checkpoint) {
      assert(Array.isArray(observation.bodies) && observation.bodies.length === expectedBodies,
        `step ${observation.step} checkpoint body states are incomplete`);
    } else {
      assert(observation.bodies === undefined, `step ${observation.step} unexpectedly stores full body state`);
    }
  }
  assert(document.observations[0].step === 0, "step-zero observation is missing");
  assert(document.observations.at(-1).step === contract.steps, "final observation is missing");

  if (contract.steps >= 5000) {
    const observedSteps = new Set(document.observations.map(({ step }) => step));
    for (const checkpoint of contract.checkpoints) {
      assert(observedSteps.has(checkpoint), `checkpoint ${checkpoint} is missing`);
    }
    if (contract.scenario === "pyramid2d-12") {
      for (let step = contract.settledStartStep; step <= contract.steps; step += 1) {
        assert(observedSteps.has(step), `settled-window step ${step} is missing`);
      }
    }
  }

  const settledObservationCount = document.observations.filter(({ step }) => step >= contract.settledStartStep).length;
  const summary = document.settledWindow;
  assert(summary?.startStep === contract.settledStartStep, "settled summary start differs");
  assert(summary.observations === settledObservationCount, "settled summary observation count differs");
  assert(summary.nearRestObservations >= 0 && summary.nearRestObservations <= summary.observations,
    "near-rest summary count is invalid");
  assert(summary.uprightObservations >= 0 && summary.uprightObservations <= summary.observations,
    "upright summary count is invalid");
  assert(summary.maxConsecutiveEnergyIncreases >= 0 &&
    summary.maxConsecutiveEnergyIncreases < Math.max(1, summary.observations),
  "energy-increase summary is invalid");

  assertFinite(document);
  return document;
}

export function readAndVerifyObservation(path, expected = {}) {
  return verifyObservation(JSON.parse(readFileSync(path, "utf8")), expected);
}
