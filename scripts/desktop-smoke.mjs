import { randomUUID } from "node:crypto";
import { rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createInterface } from "node:readline";

const root = resolve(import.meta.dirname, "..");
const scenario = process.env.PHYSBOX5_SCENARIO ?? "bootstrap";
const output = process.env.PHYSBOX5_DESKTOP_REPORT ?? "/tmp/physbox5-desktop.json";
const logPath = process.env.PHYSBOX5_DESKTOP_LOG ?? "/tmp/physbox5-desktop.log";
await rm(output, { force: true });
await rm(logPath, { force: true });
run("cargo", ["build", "-p", "physbox-desktop"]);

const executable = resolve(
  root,
  "target",
  "debug",
  process.platform === "win32" ? "physbox-desktop.exe" : "physbox-desktop",
);
const child = spawn(executable, ["--scenario", scenario], {
  cwd: root,
  stdio: ["ignore", "pipe", "pipe"],
});
const lines = [];
let adapter = null;
let stderr = "";
let settled = false;
const reader = createInterface({ input: child.stdout });

const report = await new Promise((resolveReport, rejectReport) => {
  const timeout = setTimeout(() => finishError("desktop scenario timed out"), 45_000);
  const finishError = (message) => {
    if (settled) return;
    settled = true;
    clearTimeout(timeout);
    rejectReport(new Error(message));
    child.kill();
  };

  child.stderr.on("data", (chunk) => {
    stderr += chunk;
  });
  child.on("error", (error) => finishError(`desktop launch failed: ${error.message}`));
  child.on("exit", (code, signal) => {
    if (!settled) finishError(`desktop exited before completion: code=${code} signal=${signal}`);
  });
  reader.on("line", (line) => {
    lines.push(line);
    if (line.startsWith("PHYSBOX5_MARK_ADAPTER ")) {
      adapter = JSON.parse(line.slice("PHYSBOX5_MARK_ADAPTER ".length));
      return;
    }
    if (!line.startsWith("PHYSBOX5_MARK_FRAME_RENDERED ")) return;
    const frame = JSON.parse(line.slice("PHYSBOX5_MARK_FRAME_RENDERED ".length));
    if (!frame.scenario.complete) return;
    if (frame.scenario.name !== scenario || !frame.scenario.passed) {
      finishError(`desktop shared scenario failed: ${JSON.stringify(frame.scenario)}`);
      return;
    }
    settled = true;
    clearTimeout(timeout);
    resolveReport(frame);
    child.kill();
  });
});

reader.close();
if (!child.killed) child.kill();
await writeFile(logPath, `${lines.join("\n")}\n${stderr}`);
const provenance = {
  runId: randomUUID(),
  sourceRevision: git(["rev-parse", "HEAD"]) ?? "unknown",
  sourceDirty: (git(["status", "--porcelain"]) ?? "unknown") !== "",
  host: "desktop",
  capturePhase: `${scenario}-final`,
  adapter,
  report,
};
await writeFile(output, `${JSON.stringify(provenance, null, 2)}\n`);
console.log(JSON.stringify({ output, logPath, adapter, report }, null, 2));
console.log(`PHYSBOX5_VALIDATION_SUCCESS host=desktop run_id=${provenance.runId}`);

function run(command, args) {
  const result = spawnSync(command, args, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

function git(args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "utf8" });
  return result.status === 0 ? result.stdout.trim() : undefined;
}
