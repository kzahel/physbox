#!/usr/bin/env node

import { readFileSync, renameSync, rmSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { readAndVerifyObservation } from "./observer-contract.mjs";

function fail(message) {
  console.error(`box3d-planar-observer: ${message}`);
  process.exit(1);
}

const values = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index];
  const value = process.argv[index + 1];
  if (!key?.startsWith("--") || value === undefined) {
    fail("usage: --binary PATH --scenario NAME [--steps COUNT] [--contact-recycling on|off] --output PATH");
  }
  values.set(key, value);
}

const binary = values.get("--binary");
const scenario = values.get("--scenario");
const output = values.get("--output");
const stepsText = values.get("--steps") ?? "5000";
const steps = Number(stepsText);
const contactRecyclingText = values.get("--contact-recycling") ?? "on";
const contactRecycling = contactRecyclingText === "on";
if (!binary || !scenario || !output || !Number.isInteger(steps) || steps < 0 ||
    !["on", "off"].includes(contactRecyclingText)) {
  fail("usage: --binary PATH --scenario NAME [--steps COUNT] [--contact-recycling on|off] --output PATH");
}

const first = `${output}.replay-a`;
const second = `${output}.replay-b`;
rmSync(first, { force: true });
rmSync(second, { force: true });

for (const path of [first, second]) {
  const result = spawnSync(binary, ["--scenario", scenario, "--steps", stepsText,
    "--contact-recycling", contactRecyclingText, "--output", path], {
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
  });
  if (result.status !== 0) {
    process.stderr.write(result.stdout);
    process.stderr.write(result.stderr);
    fail(`observer process failed for ${path}`);
  }
  if (!result.stderr.includes("PHYSBOX5_VALIDATION_SUCCESS")) {
    fail(`observer process omitted its success sentinel for ${path}`);
  }
  readAndVerifyObservation(path, { scenario, steps, contactRecycling });
}

const firstBytes = readFileSync(first);
const secondBytes = readFileSync(second);
if (!firstBytes.equals(secondBytes)) {
  fail("repeated deterministic observations differ byte-for-byte");
}
rmSync(output, { force: true });
renameSync(first, output);
rmSync(second, { force: true });
console.log(`PHYSBOX5_VALIDATION_SUCCESS source=box3d scenario=${scenario} steps=${steps} contact_recycling=${contactRecyclingText} deterministic_replays=2 output=${output}`);
