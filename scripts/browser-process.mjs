import { spawnSync } from "node:child_process";
import { chromium } from "@playwright/test";

export async function launchManagedBrowser(options) {
  const initialCrashpadPids = chromeCrashpadPids();
  let browserServer;
  try {
    browserServer = await chromium.launchServer(options);
    const browser = await chromium.connect(browserServer.wsEndpoint());
    return { browser, browserServer, initialCrashpadPids };
  } catch (error) {
    const browserKill = browserServer?.kill();
    terminateNewCrashpadHelpers(initialCrashpadPids);
    await browserKill;
    throw error;
  }
}

export async function closeManagedBrowser(session) {
  const browserKill = session?.browserServer.kill();
  terminateNewCrashpadHelpers(session?.initialCrashpadPids ?? new Set());
  await browserKill;
}

export async function closeHttpServer(server) {
  await new Promise((resolveClose) => {
    server.close(resolveClose);
    server.closeAllConnections();
  });
}

function chromeCrashpadPids() {
  if (process.platform !== "darwin") return new Set();
  const result = spawnSync("ps", ["-axo", "pid=,command="], { encoding: "utf8" });
  if (result.status !== 0) return new Set();
  return new Set(
    result.stdout
      .split("\n")
      .filter((line) => line.includes("/Helpers/chrome_crashpad_handler "))
      .map((line) => Number.parseInt(line.trimStart(), 10))
      .filter(Number.isSafeInteger),
  );
}

function terminateNewCrashpadHelpers(initialPids) {
  const spawnedPids = [...chromeCrashpadPids()].filter((pid) => !initialPids.has(pid));
  for (const pid of spawnedPids) {
    try {
      process.kill(pid, "SIGTERM");
    } catch (error) {
      if (error?.code !== "ESRCH") throw error;
    }
  }
  if (spawnedPids.length) {
    console.warn(
      `terminated ${spawnedPids.length} Chrome crashpad helper(s) retained during Playwright shutdown`,
    );
  }
}
