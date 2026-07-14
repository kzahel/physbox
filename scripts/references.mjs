import { spawnSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const manifestPath = path.join(root, "reference", "pins.toml");
const referenceRoot = path.join(root, "reference");

function fail(message) {
  console.error(`reference error: ${message}`);
  process.exit(1);
}

function parseValue(source, lineNumber) {
  if (source === "true") return true;
  if (source === "false") return false;
  if (/^\d+$/.test(source)) return Number(source);
  if (source.startsWith('"') || source.startsWith("[")) {
    try {
      return JSON.parse(source);
    } catch (error) {
      fail(`invalid value on pins.toml line ${lineNumber}: ${error.message}`);
    }
  }
  fail(`unsupported value on pins.toml line ${lineNumber}`);
}

function readManifest() {
  const manifest = { checkout: [], sibling: [] };
  let current = manifest;

  const lines = fs.readFileSync(manifestPath, "utf8").split(/\r?\n/);
  for (const [index, rawLine] of lines.entries()) {
    const line = rawLine.trim();
    if (line === "" || line.startsWith("#")) continue;

    const section = line.match(/^\[\[(checkout|sibling)\]\]$/);
    if (section) {
      current = { kind: section[1] };
      manifest[section[1]].push(current);
      continue;
    }

    const assignment = line.match(/^([a-z_]+)\s*=\s*(.+)$/);
    if (!assignment) fail(`cannot parse pins.toml line ${index + 1}`);
    current[assignment[1]] = parseValue(assignment[2], index + 1);
  }

  if (manifest.schema_version !== 1) fail("pins.toml schema_version must be 1");

  const names = new Set();
  const locations = new Set();
  for (const record of [...manifest.checkout, ...manifest.sibling]) {
    for (const key of ["name", "path", "repository", "revision"]) {
      if (typeof record[key] !== "string" || record[key] === "") {
        fail(`${record.kind} record is missing ${key}`);
      }
    }
    if (!/^[0-9a-f]{40}$/.test(record.revision)) {
      fail(`${record.name} does not have a full lowercase Git revision`);
    }
    if (names.has(record.name)) fail(`duplicate name ${record.name}`);
    names.add(record.name);

    record.absolutePath = path.resolve(root, record.path);
    if (locations.has(record.absolutePath)) fail(`duplicate path ${record.path}`);
    locations.add(record.absolutePath);

    if (
      record.kind === "checkout" &&
      !record.absolutePath.startsWith(`${referenceRoot}${path.sep}`)
    ) {
      fail(`${record.name} checkout must remain below reference/`);
    }
    if (
      record.allowed_dirty_paths !== undefined &&
      (!Array.isArray(record.allowed_dirty_paths) ||
        record.allowed_dirty_paths.some((value) => typeof value !== "string"))
    ) {
      fail(`${record.name} allowed_dirty_paths must be an array of strings`);
    }
  }

  return manifest;
}

function git(args, { capture = true, allowFailure = false } = {}) {
  const result = spawnSync("git", args, {
    cwd: root,
    encoding: capture ? "utf8" : undefined,
    stdio: capture ? "pipe" : "inherit",
  });
  if (result.error) fail(`could not run git: ${result.error.message}`);
  if (result.status !== 0 && !allowFailure) {
    const detail = capture ? (result.stderr || result.stdout).trim() : "";
    fail(`${args.join(" ")} failed${detail ? `: ${detail}` : ""}`);
  }
  return result;
}

function gitAt(record, args, options) {
  return git(["-C", record.absolutePath, ...args], options);
}

function outputAt(record, args) {
  return gitAt(record, args).stdout.trim();
}

function normalizeRemote(remote) {
  return remote.trim().replace(/\/+$/, "").replace(/\.git$/, "");
}

function dirtyPaths(record) {
  const output = gitAt(record, [
    "status",
    "--porcelain=v1",
    "--untracked-files=all",
  ]).stdout.trimEnd();
  if (!output) return [];
  return output.split("\n").map((line) => {
    const pathText = line.slice(3);
    const renameParts = pathText.split(" -> ");
    return renameParts[renameParts.length - 1];
  });
}

function repositoryProblem(record) {
  if (!fs.existsSync(record.absolutePath)) {
    return record.required === false ? null : "missing";
  }

  const topLevel = gitAt(record, ["rev-parse", "--show-toplevel"], {
    allowFailure: true,
  });
  if (topLevel.status !== 0) return "not a Git repository";
  if (path.resolve(topLevel.stdout.trim()) !== fs.realpathSync(record.absolutePath)) {
    return "is not the root of its own Git repository";
  }

  const head = outputAt(record, ["rev-parse", "HEAD"]);
  if (head !== record.revision) {
    return `HEAD is ${head}; expected ${record.revision}`;
  }

  const remote = gitAt(record, ["remote", "get-url", "origin"], {
    allowFailure: true,
  });
  if (remote.status !== 0) return "has no origin remote";
  if (normalizeRemote(remote.stdout) !== normalizeRemote(record.repository)) {
    return `origin is ${remote.stdout.trim()}; expected ${record.repository}`;
  }

  const dirty = dirtyPaths(record);
  const allowed = new Set(record.allowed_dirty_paths || []);
  const unexpected = dirty.filter((entry) => !allowed.has(entry));
  if (unexpected.length > 0) {
    return `has unexpected working-tree changes: ${unexpected.join(", ")}`;
  }
  if (dirty.length > 0) {
    return { warning: `known working-tree changes: ${dirty.join(", ")}` };
  }
  return null;
}

function status(manifest) {
  let failed = false;
  for (const record of [...manifest.checkout, ...manifest.sibling]) {
    const problem = repositoryProblem(record);
    if (typeof problem === "string") {
      console.error(`[fail] ${record.name}: ${problem}`);
      failed = true;
    } else if (problem?.warning) {
      console.log(`[warn] ${record.name}: ${problem.warning}`);
    } else {
      console.log(`[ok]   ${record.name}: ${record.revision}`);
    }
  }
  if (failed) process.exit(1);
}

function syncCheckout(record) {
  let wasCloned = false;
  if (!fs.existsSync(record.absolutePath)) {
    fs.mkdirSync(path.dirname(record.absolutePath), { recursive: true });
    console.log(`[clone] ${record.name} -> ${record.path}`);
    git(
      [
        "clone",
        "--filter=blob:none",
        "--no-checkout",
        record.repository,
        record.absolutePath,
      ],
      { capture: false },
    );
    wasCloned = true;
  } else {
    const probe = gitAt(record, ["rev-parse", "--show-toplevel"], {
      allowFailure: true,
    });
    if (probe.status !== 0) fail(`${record.path} exists but is not a Git repository`);
    if (dirtyPaths(record).length > 0) {
      fail(`${record.path} has local changes; refusing to replace them`);
    }
  }

  const remote = outputAt(record, ["remote", "get-url", "origin"]);
  if (normalizeRemote(remote) !== normalizeRemote(record.repository)) {
    fail(`${record.path} origin is ${remote}; expected ${record.repository}`);
  }

  let object = gitAt(record, ["cat-file", "-e", `${record.revision}^{commit}`], {
    allowFailure: true,
  });
  if (object.status !== 0) {
    console.log(`[fetch] ${record.name} ${record.revision}`);
    gitAt(record, ["fetch", "--filter=blob:none", "origin", record.revision], {
      capture: false,
    });
    object = gitAt(record, ["cat-file", "-e", `${record.revision}^{commit}`], {
      allowFailure: true,
    });
    if (object.status !== 0) fail(`${record.name} revision was not fetched`);
  }

  if (wasCloned || outputAt(record, ["rev-parse", "HEAD"]) !== record.revision) {
    console.log(`[pin]   ${record.name} ${record.revision}`);
    gitAt(record, ["checkout", "--detach", record.revision], { capture: false });
  }
}

const command = process.argv[2];
if (!new Set(["status", "sync"]).has(command)) {
  fail("usage: node scripts/references.mjs <status|sync>");
}

const manifest = readManifest();
if (command === "sync") {
  for (const record of manifest.checkout) syncCheckout(record);
}
status(manifest);
