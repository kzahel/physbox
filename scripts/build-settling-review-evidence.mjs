#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { gunzipSync } from "node:zlib";
import { dirname, relative, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const repository = resolve(scriptDirectory, "..");
const arguments_ = parseArguments(process.argv.slice(2));
const outputDirectory = resolve(repository, arguments_.output);

const inputs = {
  factorial:
    "evidence/p5.2b-s6.4-soft-contact/manifest.json",
  authority:
    "evidence/p5.2b-s6.4-soft-contact/authority-performance/manifest.json",
  failures:
    "evidence/p5.2b-s6.4-soft-contact/failures/manifest.json",
  crossHost:
    "evidence/p5.2b-s6.4-settling-cross-host/manifest.json",
  hardTrace:
    "evidence/p5.2b-s6.4-soft-contact/full-traces/centered-stack-8-hard-direct-control.json.gz",
  candidateTrace:
    "evidence/p5.2b-s6.4-soft-contact/full-traces/centered-stack-8-soft-hz30-d10-static1-prior.json.gz",
  avdHard16:
    "evidence/p5.2b-s6.4-settling-cross-host/platform/avd-settling-hard-16.png",
  avdCandidate16:
    "evidence/p5.2b-s6.4-settling-cross-host/platform/avd-settling-candidate-16.png",
  candidate8Report:
    "evidence/p5.2b-s6.4-settling-cross-host/platform/headless-settling-candidate-8.json",
  candidate16Report:
    "evidence/p5.2b-s6.4-settling-cross-host/platform/headless-settling-candidate-16.json",
};

const box2dCheckpoints = [600, 900, 1200].map((step) => ({
  step,
  path:
    step === 1200
      ? "evidence/p5.2b-s6.4-soft-contact/authority-performance/oracles/p5.2b-s6.4-authority-20260717-centered-stack-8-1200-primary/oracle-result.json"
      : `evidence/p5.2b-s6.4-soft-contact/authority-performance/oracles/p5.2b-s6.4-authority-20260717-centered-stack-8-${String(step).padStart(4, "0")}-checkpoint/oracle-result.json`,
}));

const status = execFileSync("git", ["status", "--porcelain=v1", "--untracked-files=all"], {
  cwd: repository,
  encoding: "utf8",
});
const sourceDirty = status.length !== 0;
if (sourceDirty && !arguments_.allowDirty) {
  fail("refusing to assemble review evidence from a dirty source tree");
}
if (existsSync(outputDirectory)) {
  fail(`refusing to replace existing output directory ${outputDirectory}`);
}

const factorial = readJson(inputs.factorial);
const authority = readJson(inputs.authority);
const failures = readJson(inputs.failures);
const crossHost = readJson(inputs.crossHost);
validateInputs({ factorial, authority, failures, crossHost });

const candidate = factorial.finalistSummaries.find(
  (entry) => entry.candidate.id === "soft-hz30-d10-static1-prior",
);
const hard = factorial.finalistSummaries.find(
  (entry) => entry.candidate.id === "hard-direct-control",
);
const candidateTiming = authority.gpuPerformance.find(
  (entry) => entry.candidate.id === candidate.candidate.id && entry.bodies === 1024,
);
const hardTiming = authority.gpuPerformance.find(
  (entry) => entry.candidate.id === hard.candidate.id && entry.bodies === 1024,
);
const box2dTiming = authority.optimizedBox2dPerformance
  .filter((entry) => entry.bodies === 1024)
  .sort((left, right) => left.p50Seconds - right.p50Seconds)[0];
const box2dStack16 = authority.authority
  .find((entry) => entry.label === "centered-stack-16")
  .samples.find((sample) => sample.phase === "primary");

const playback = buildPlayback();
const decision = {
  status: "hold-promotion",
  promotionAuthorized: false,
  recommendation:
    "Keep the candidate as the laboratory leader, do not promote it, and authorize a targeted cross-backend diagnosis and minimization of the AVD 16-stack collapse before retuning or repeating S6.5.",
  candidate: {
    ...candidate.candidate,
    descriptorSha256: sha256(Buffer.from(stableJson(candidate.candidate))),
    directScenarioHashes: Object.fromEntries(
      [
        ["settling-candidate-8", inputs.candidate8Report],
        ["settling-candidate-16", inputs.candidate16Report],
      ].map(([scenario, path]) => {
        const report = readJson(path);
        return [scenario, report.report.sceneContract.scenarioHash];
      }),
    ),
  },
  objectiveMatrix: [
    {
      lane: "hard-direct-control",
      role: "current product",
      hardGates: `${hard.hardGatePasses}/${hard.sceneCount}`,
      metalStructures: `${hard.structuralPasses}/${hard.sceneCount}`,
      energyEnvelopes: `${hard.energyEnvelopePasses}/${hard.sceneCount}`,
      settledChurn: hard.settledChurn,
      worstLinearSpeedMps: hard.maximumSettledLinearSpeed,
      crossHostStructures: "6/6",
      p50At1024Ms: hardTiming.gpuEndToEndP50Seconds * 1000,
    },
    {
      lane: candidate.candidate.id,
      role: "laboratory leader",
      hardGates: `${candidate.hardGatePasses}/${candidate.sceneCount}`,
      metalStructures: `${candidate.structuralPasses}/${candidate.sceneCount}`,
      energyEnvelopes: `${candidate.energyEnvelopePasses}/${candidate.sceneCount}`,
      settledChurn: candidate.settledChurn,
      worstLinearSpeedMps: candidate.maximumSettledLinearSpeed,
      crossHostStructures: "5/6",
      p50At1024Ms: candidateTiming.gpuEndToEndP50Seconds * 1000,
    },
    {
      lane: "box2d-v3",
      role: "rigid behavior authority / optimized CPU timing",
      hardGates: "11/11 authority exact",
      metalStructures: "n/a",
      energyEnvelopes: "n/a",
      settledChurn: null,
      worstLinearSpeedMps: box2dStack16.dynamicLinearSpeedMax,
      crossHostStructures: "native oracle only",
      p50At1024Ms: box2dTiming.p50Seconds * 1000,
    },
  ],
  cost: {
    candidateP50At1024Ms: candidateTiming.gpuEndToEndP50Seconds * 1000,
    hardP50At1024Ms: hardTiming.gpuEndToEndP50Seconds * 1000,
    optimizedBox2dP50At1024Ms: box2dTiming.p50Seconds * 1000,
    candidateVersusHardMs:
      (candidateTiming.gpuEndToEndP50Seconds - hardTiming.gpuEndToEndP50Seconds) * 1000,
    candidateVersusBox2dRatio:
      candidateTiming.gpuEndToEndP50Seconds / box2dTiming.p50Seconds,
    candidateDispatchDelta: 1,
    candidateAllocationDeltaBytes: 0,
    inside60HzOnMeasuredMetal: candidateTiming.gpuEndToEndP50Seconds < 1 / 60,
    cpuCrossoverClaimSupported: false,
    liquidHeadroomClaimSupported: false,
  },
  knownFailures: {
    avdCandidate16Collapsed: true,
    crossHostStructureGatePassed: crossHost.crossHostStructureGatePassed,
    firstEnergyFailureStackDepth: failures.minimumFailingStackCount,
    stack16PositiveEnergyRun: failures.stackSweep.find(
      (entry) => entry.label === "centered-stack-16",
    ).positiveEnergyRunMax,
    minimizedMixedPileDynamicBodies: failures.mixedPileMinimized.dynamicBodyCount,
    minimizedMixedPilePositiveEnergyRun: failures.mixedPileMinimized.positiveEnergyRunMax,
    physicalDeviceAvailable: false,
  },
  rejectedAlternatives: [
    "stateless relative-coherence contact identity: catastrophic 16-stack loss",
    "canonical-coherence contact identity: catastrophic 16-stack loss",
    "identity-only prior-contact hysteresis: insufficient deep-stack quality",
    "30 Hz / damping 10 / static×2 combined lane: materially worse 16-stack composition",
  ],
  reconsiderationTriggers: [
    "the AVD candidate-16 divergence is minimized, explained, and removed without losing portability",
    "all frozen awake structures and realistic fixtures pass the nonincreasing-energy gate",
    "native Metal, Chrome WebGPU, hardware-backed AVD, and an available physical device preserve the same structures",
    "any coefficient, identity band, scheduler, solver, tolerance, or authority change causes the complete matrix to be rerun",
  ],
};

mkdirSync(outputDirectory, { recursive: false });
writeFileSync(resolve(outputDirectory, "playback.json"), `${stableJson(playback)}\n`);
writeFileSync(resolve(outputDirectory, "playback.svg"), renderPlayback(playback, decision));
writeFileSync(resolve(outputDirectory, "README.md"), renderReadme(decision));
writeFileSync(
  resolve(outputDirectory, "verify.mjs"),
  `#!/usr/bin/env node\nimport { verifySettlingReviewEvidence } from "../../scripts/verify-settling-review-evidence.mjs";\nawait verifySettlingReviewEvidence(new URL(".", import.meta.url));\n`,
);

const manifest = {
  schemaVersion: 1,
  artifactKind: "p5.2b-s6.5-settling-review",
  runId: arguments_.runId,
  sourceRevision: execFileSync("git", ["rev-parse", "HEAD"], {
    cwd: repository,
    encoding: "utf8",
  }).trim(),
  sourceDirty,
  productDefaultsChanged: false,
  physicalDefaultsChanged: false,
  numericalTolerancesChanged: false,
  oraclePrecedenceChanged: false,
  decision,
  inputEvidence: [
    ...Object.entries(inputs).map(([id, path]) => artifactRecord(id, path)),
    ...box2dCheckpoints.map(({ step, path }) => artifactRecord(`box2d-${step}`, path)),
  ],
  artifacts: ["playback.json", "playback.svg"].map((path) => ({
    path,
    sha256: sha256(readFileSync(resolve(outputDirectory, path))),
  })),
};
writeFileSync(resolve(outputDirectory, "manifest.json"), `${stableJson(manifest)}\n`);

console.log(
  `PHYSBOX5_VALIDATION_SUCCESS host=evidence scenario=s6.5-settling-review run_id=${arguments_.runId} output=${relative(repository, outputDirectory)}`,
);

function parseArguments(values) {
  const result = {
    output: "evidence/p5.2b-s6.5-settling-review",
    runId: "p5.2b-s6.5-settling-review-20260717",
    allowDirty: false,
  };
  for (let index = 0; index < values.length; index += 1) {
    const value = values[index];
    if (value === "--allow-dirty") {
      result.allowDirty = true;
    } else if (value === "--output" || value === "--run-id") {
      const next = values[index + 1];
      if (!next) fail(`missing value for ${value}`);
      index += 1;
      if (value === "--output") result.output = next;
      else result.runId = next;
    } else {
      fail(`unknown argument ${value}`);
    }
  }
  return result;
}

function readJson(path) {
  return JSON.parse(readFileSync(resolve(repository, path), "utf8"));
}

function readGzipJson(path) {
  return JSON.parse(gunzipSync(readFileSync(resolve(repository, path))));
}

function validateInputs(values) {
  const expected = {
    factorial: "p5.2b-s6.4-soft-contact-factorial",
    authority: "p5.2b-s6.4-soft-contact-authority-performance",
    failures: "p5.2b-s6.4-soft-contact-failure-minima",
    crossHost: "p5.2b-s6.4-settling-cross-host",
  };
  for (const [key, artifactKind] of Object.entries(expected)) {
    if (values[key].artifactKind !== artifactKind) {
      fail(`unexpected ${key} artifact kind ${values[key].artifactKind}`);
    }
    if (values[key].sourceDirty) fail(`${key} evidence is dirty`);
    if (values[key].productConstructorChanged) fail(`${key} changed the product constructor`);
  }
  if (values.crossHost.crossHostStructureGatePassed !== false) {
    fail("cross-host package no longer preserves the promotion-blocking result");
  }
}

function buildPlayback() {
  const checkpoints = [600, 900, 1200];
  const gpuLane = (id, path) => {
    const trace = readGzipJson(path);
    return {
      id,
      authority: "PhysBox 5 generated-contact GPU",
      sourceScenarioHash: trace.sourceScenarioHash,
      policy: trace.policy,
      samples: checkpoints.map((step) => {
        const observation = trace.observations.find(
          (entry) => entry.base.completedSteps === step,
        );
        if (!observation) fail(`missing ${id} observation at step ${step}`);
        return {
          step,
          linearSpeedMax: observation.base.residualLinearSpeedMax,
          angularEdgeSpeedMax: observation.angularEdgeSpeedMax,
          kineticEnergy: observation.base.kineticEnergy,
          stackHeight: observation.base.stackHeight,
          penetrationMax: observation.base.penetrationMax,
          contactFeatureChurn: observation.base.contactFeatureChurn,
        };
      }),
    };
  };
  const box2d = {
    id: "box2d-v3",
    authority: "clean pinned native Box2D v3 rigid authority",
    revision: authority.box2d.revision,
    samples: box2dCheckpoints.map(({ step, path }) => {
      const result = readJson(path);
      const dynamic = result.bodies.filter((body) => body.inverseMass > 0);
      return {
        step,
        linearSpeedMax: Math.max(
          ...dynamic.map((body) => Math.hypot(body.linearVelocity.x, body.linearVelocity.y)),
        ),
        angularEdgeSpeedMax: Math.max(
          ...dynamic.map(
            (body) =>
              Math.abs(body.angularVelocity) *
              Math.hypot(body.shape.half_extents.x, body.shape.half_extents.y),
          ),
        ),
        kineticEnergy: dynamic.reduce((total, body) => {
          const mass = 1 / body.inverseMass;
          const inertia = 1 / body.inverseInertia;
          return (
            total +
            0.5 * mass * (body.linearVelocity.x ** 2 + body.linearVelocity.y ** 2) +
            0.5 * inertia * body.angularVelocity ** 2
          );
        }, 0),
        stackHeight: Math.max(
          ...dynamic.map((body) => body.position.y + body.shape.half_extents.y),
        ),
        penetrationMax: null,
        contactFeatureChurn: null,
        resultPayloadHash: authority.authority
          .find((entry) => entry.label === "centered-stack-8")
          .samples.find((sample) => sample.completedSteps === step && sample.phase !== "replay")
          .resultPayloadHash,
      };
    }),
  };
  return {
    schemaVersion: 1,
    artifactKind: "p5.2b-s6.5-three-way-settling-playback",
    fixture: "centered-stack-8",
    fixedStepHz: 60,
    sleepingEnabled: false,
    checkpoints,
    lanes: [
      gpuLane("hard-direct-control", inputs.hardTrace),
      gpuLane("soft-hz30-d10-static1-prior", inputs.candidateTrace),
      box2d,
    ],
  };
}

function renderPlayback(playback_, decision_) {
  const width = 1440;
  const laneWidth = 430;
  const laneXs = [40, 505, 970];
  const labels = ["CURRENT HARD CONTROL", "SETTLING CANDIDATE", "BOX2D v3 AUTHORITY"];
  const colors = ["#f59e0b", "#22c55e", "#60a5fa"];
  const cards = playback_.lanes
    .map((lane, laneIndex) => {
      const x = laneXs[laneIndex];
      const samples = lane.samples
        .map((sample, sampleIndex) => {
          const y = 174 + sampleIndex * 142;
          const velocityWidth = logBar(sample.linearSpeedMax, 235);
          return `
    <rect x="${x}" y="${y}" width="${laneWidth}" height="122" rx="10" class="sample"/>
    <text x="${x + 18}" y="${y + 27}" class="step">step ${sample.step}</text>
    <text x="${x + 18}" y="${y + 54}" class="metric">max speed  ${formatScientific(sample.linearSpeedMax)} m/s</text>
    <rect x="${x + 177}" y="${y + 42}" width="235" height="12" rx="6" class="bar-bg"/>
    <rect x="${x + 177}" y="${y + 42}" width="${velocityWidth}" height="12" rx="6" fill="${colors[laneIndex]}"/>
    <text x="${x + 18}" y="${y + 80}" class="metric">kinetic E  ${formatScientific(sample.kineticEnergy)}</text>
    <text x="${x + 18}" y="${y + 105}" class="metric">stack top  ${sample.stackHeight.toFixed(6)} m</text>`;
        })
        .join("");
      return `
  <rect x="${x}" y="95" width="${laneWidth}" height="520" rx="14" class="lane"/>
  <text x="${x + 18}" y="128" fill="${colors[laneIndex]}" class="lane-title">${labels[laneIndex]}</text>
  <text x="${x + 18}" y="151" class="lane-meta">${escapeXml(lane.id)}</text>${samples}`;
    })
    .join("");
  const hardImage = readFileSync(resolve(repository, inputs.avdHard16)).toString("base64");
  const candidateImage = readFileSync(resolve(repository, inputs.avdCandidate16)).toString("base64");
  return `<svg xmlns="http://www.w3.org/2000/svg" width="${width}" height="1180" viewBox="0 0 ${width} 1180">
<style>
  text { font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace; fill: #e5e7eb; }
  .title { font: 700 28px ui-sans-serif, system-ui, sans-serif; }
  .subtitle { font: 15px ui-sans-serif, system-ui, sans-serif; fill: #9ca3af; }
  .lane { fill: #111827; stroke: #374151; stroke-width: 1; }
  .sample { fill: #0b1220; stroke: #243044; }
  .lane-title { font-weight: 700; font-size: 16px; }
  .lane-meta { font-size: 12px; fill: #9ca3af; }
  .step { font-weight: 700; font-size: 14px; }
  .metric { font-size: 13px; fill: #d1d5db; }
  .bar-bg { fill: #1f2937; }
  .warning { fill: #2a1116; stroke: #ef4444; stroke-width: 2; }
  .warning-title { font: 700 20px ui-sans-serif, system-ui, sans-serif; fill: #fca5a5; }
  .caption { font: 14px ui-sans-serif, system-ui, sans-serif; fill: #d1d5db; }
  .recommendation { font: 700 17px ui-sans-serif, system-ui, sans-serif; fill: #fca5a5; }
</style>
<rect width="1440" height="1180" fill="#070b12"/>
<text x="40" y="42" class="title">S6.5 awake-settling review</text>
<text x="40" y="69" class="subtitle">centered 8-stack · synchronized exact steps · sleeping disabled · log-scaled speed bars</text>
${cards}
<rect x="40" y="640" width="1360" height="455" rx="14" class="warning"/>
<text x="62" y="676" class="warning-title">PROMOTION BLOCKER · hardware-backed AVD Vulkan · candidate 16-stack collapsed at captured step 1203</text>
<text x="62" y="704" class="caption">The same shared candidate remains centered on Metal and Chrome; its AVD hard control remains centered.</text>
<text x="62" y="732" class="caption">AVD hard/direct 16 · centered</text>
<text x="746" y="732" class="caption">AVD candidate 16 · collapsed</text>
<image x="62" y="742" width="632" height="330" preserveAspectRatio="xMidYMid meet" href="data:image/png;base64,${hardImage}"/>
<image x="746" y="742" width="632" height="330" preserveAspectRatio="xMidYMid meet" href="data:image/png;base64,${candidateImage}"/>
<text x="40" y="1128" class="recommendation">Recommendation: HOLD PROMOTION. Keep this candidate as the laboratory leader.</text>
<text x="40" y="1155" class="recommendation">Next: minimize and explain the AVD 16-stack divergence, then repeat the cross-host gate.</text>
</svg>\n`;
}

function renderReadme(decision_) {
  const [hardRow, candidateRow, box2dRow] = decision_.objectiveMatrix;
  return `# S6.5 awake-settling decision package

Status: **hold promotion**. No product default, physical coefficient,
numerical tolerance, or oracle precedence changes in this package.

## Recommendation

${decision_.recommendation}

The candidate is a large visible quality improvement on Metal and is nearly
cost-neutral against the current GPU path. It is not safe to promote: the
same 16-stack that remains centered on native Metal and Chrome collapses on
the hardware-backed \`jstorrent-tablet\` AVD, while the hard control remains
centered. The retained 16-stack and minimized four-body mixed pile also have
rising-energy modes. This is a portability/quality stop, not a request to
widen a gate.

## Candidate identity

- lane: \`${decision_.candidate.id}\`
- contact identity: \`${decision_.candidate.identityPolicy}\`
- contact softness: \`${decision_.candidate.softness.contactHertz} Hz\`, damping
  ratio \`${decision_.candidate.softness.dampingRatio}\`, static multiplier
  \`${decision_.candidate.softness.staticContactHertzMultiplier}\`
- descriptor SHA-256: \`${decision_.candidate.descriptorSha256}\`
- direct 8-stack scenario: \`${decision_.candidate.directScenarioHashes["settling-candidate-8"]}\`
- direct 16-stack scenario: \`${decision_.candidate.directScenarioHashes["settling-candidate-16"]}\`

## Objective matrix

| Lane | Hard gates | Metal structure | Energy | Settled churn | Worst speed | Cross-host structure | 1K P50 |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| current hard/direct | ${hardRow.hardGates} | ${hardRow.metalStructures} | ${hardRow.energyEnvelopes} | ${hardRow.settledChurn} | ${hardRow.worstLinearSpeedMps.toFixed(6)} m/s | ${hardRow.crossHostStructures} | ${hardRow.p50At1024Ms.toFixed(3)} ms |
| candidate | ${candidateRow.hardGates} | ${candidateRow.metalStructures} | ${candidateRow.energyEnvelopes} | ${candidateRow.settledChurn} | ${candidateRow.worstLinearSpeedMps.toFixed(6)} m/s | **${candidateRow.crossHostStructures}** | ${candidateRow.p50At1024Ms.toFixed(3)} ms |
| Box2D v3 authority | ${box2dRow.hardGates} | ${box2dRow.metalStructures} | ${box2dRow.energyEnvelopes} | n/a | ${box2dRow.worstLinearSpeedMps.toFixed(6)} m/s | ${box2dRow.crossHostStructures} | ${box2dRow.p50At1024Ms.toFixed(3)} ms |

The candidate costs ${decision_.cost.candidateVersusHardMs.toFixed(3)} ms more
than hard/direct at 1K in matched Metal timing, adds one dispatch, and adds no
allocation. Its ${decision_.cost.candidateP50At1024Ms.toFixed(3)} ms complete
step fits 60 Hz on that host, but it is
${decision_.cost.candidateVersusBox2dRatio.toFixed(1)}× optimized four-worker
Box2D. This supports no CPU-crossover or liquid-headroom claim.

## Synchronized review

- [Three-way playback](playback.svg): current, candidate, and Box2D at exact
  steps 600, 900, and 1200 on the same centered 8-stack, followed by the AVD
  hard/candidate 16-stack captures.
- [Machine-readable playback](playback.json)
- [AVD hard 16 capture](../p5.2b-s6.4-settling-cross-host/platform/avd-settling-hard-16.png)
- [AVD candidate 16 collapse](../p5.2b-s6.4-settling-cross-host/platform/avd-settling-candidate-16.png)

## Known minimized failures

- AVD candidate 16 collapses by captured step 1203; candidate 8 and both hard
  controls preserve structure. Native Metal and Chrome preserve candidate 16.
- The first candidate stack energy-envelope failure appears at depth 9, with
  a non-monotonic boundary. The governed 16-stack has
  ${decision_.knownFailures.stack16PositiveEnergyRun} consecutive positive
  intervals and grows from \`4.73e-7\` to \`1.18e-3\` kinetic energy.
- The mixed-pile reproducer is minimized from eight to
  ${decision_.knownFailures.minimizedMixedPileDynamicBodies} dynamic bodies
  and retains ${decision_.knownFailures.minimizedMixedPilePositiveEnergyRun}
  consecutive rising intervals.
- No physical Android device was connected. This is an availability gap, not
  a passing platform result.

## Rejected alternatives

${decision_.rejectedAlternatives.map((entry) => `- ${entry}`).join("\n")}

## Direct review recipes

These names are launch-only comparators; they are not menu entries or product
defaults.

\`\`\`sh
cargo run -r --bin physbox-desktop -- --scenario settling-hard-8
cargo run -r --bin physbox-desktop -- --scenario settling-candidate-8
cargo run -r --bin physbox-desktop -- --scenario settling-hard-16
cargo run -r --bin physbox-desktop -- --scenario settling-candidate-16

cargo run -r -p physbox-headless -- --scenario settling-candidate-16 \\
  --output /tmp/settling-candidate-16.png \\
  --report /tmp/settling-candidate-16.json

PHYSBOX5_SCENARIO=settling-candidate-16 npm run test:web
android/validate-avd.sh --skip-build --scenario settling-candidate-16
\`\`\`

After the reviewed commit is deliberately deployed, the corresponding URLs
are:

- \`https://physbox5.kzahel.com/?scenario=settling-hard-8\`
- \`https://physbox5.kzahel.com/?scenario=settling-candidate-8\`
- \`https://physbox5.kzahel.com/?scenario=settling-hard-16\`
- \`https://physbox5.kzahel.com/?scenario=settling-candidate-16\`

## Reconsideration triggers

${decision_.reconsiderationTriggers.map((entry) => `- ${entry}`).join("\n")}

## Verification

\`\`\`sh
node evidence/p5.2b-s6.5-settling-review/verify.mjs
\`\`\`
`;
}

function artifactRecord(id, path) {
  return { id, path, sha256: sha256(readFileSync(resolve(repository, path))) };
}

function stableJson(value) {
  return JSON.stringify(sortValue(value), null, 2);
}

function sortValue(value) {
  if (Array.isArray(value)) return value.map(sortValue);
  if (value && typeof value === "object") {
    return Object.fromEntries(
      Object.keys(value)
        .sort()
        .map((key) => [key, sortValue(value[key])]),
    );
  }
  return value;
}

function sha256(bytes) {
  return createHash("sha256").update(bytes).digest("hex");
}

function formatScientific(value) {
  return value.toExponential(3);
}

function logBar(value, maximum) {
  const normalized = Math.max(0, Math.min(1, (Math.log10(Math.max(value, 1e-8)) + 8) / 8));
  return Math.max(2, normalized * maximum).toFixed(1);
}

function escapeXml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function fail(message) {
  throw new Error(message);
}
