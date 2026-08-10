import { createHash } from "node:crypto";
import { readFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { spawnSync } from "node:child_process";

export async function sourceState(root) {
  const sourceRevision = gitText(root, ["rev-parse", "HEAD"]);
  if (!sourceRevision) throw new Error("source provenance requires a Git revision");
  const status = gitBytes(root, ["status", "--porcelain=v1", "-z"]);
  if (status === undefined) throw new Error("source provenance could not read Git status");
  const sourceDirty = status.length !== 0;
  return {
    sourceRevision,
    sourceDirty,
    sourcePatchDigest: sourceDirty ? await dirtyPatchDigest(root, status) : undefined,
  };
}

async function dirtyPatchDigest(root, status) {
  const hash = createHash("sha256");
  hash.update(status);
  const diff = gitBytes(root, ["diff", "--binary", "HEAD"]);
  const untracked = gitBytes(root, ["ls-files", "--others", "--exclude-standard", "-z"]);
  if (diff === undefined || untracked === undefined) {
    throw new Error("source provenance could not hash the dirty source");
  }
  hash.update(diff);
  for (const path of untracked.toString("utf8").split("\0").filter(Boolean)) {
    hash.update(path);
    hash.update(await readFile(resolve(root, path)));
  }
  return hash.digest("hex");
}

function gitText(root, args) {
  const bytes = gitBytes(root, args);
  return bytes === undefined ? undefined : bytes.toString("utf8").trim();
}

function gitBytes(root, args) {
  const result = spawnSync("git", args, { cwd: root, encoding: "buffer" });
  return result.status === 0 ? result.stdout : undefined;
}

if (process.argv[1] && import.meta.url === pathToFileURL(process.argv[1]).href) {
  const root = resolve(process.argv[2] ?? process.cwd());
  console.log(JSON.stringify(await sourceState(root)));
}
