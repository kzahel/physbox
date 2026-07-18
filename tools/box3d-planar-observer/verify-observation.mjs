#!/usr/bin/env node

import { readAndVerifyObservation } from "./observer-contract.mjs";

const path = process.argv[2];
if (!path || process.argv.length !== 3) {
  console.error("usage: verify-observation.mjs PATH");
  process.exit(1);
}
const document = readAndVerifyObservation(path);
console.log(
  `PHYSBOX5_VALIDATION_SUCCESS source=box3d scenario=${document.contract.scenario} steps=${document.contract.steps} contact_recycling=${document.contract.contactRecycling ? "on" : "off"} observations=${document.observations.length}`,
);
