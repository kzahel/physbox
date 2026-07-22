import assert from "node:assert/strict";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import test from "node:test";

const root = resolve(import.meta.dirname, "..");
const hostEnv = resolve(import.meta.dirname, "host-env.sh");
const setup = resolve(import.meta.dirname, "setup-host.sh");
const withHostEnv = resolve(import.meta.dirname, "with-host-env.sh");

test("host platform mapping is explicit", () => {
  const result = bash(`source "${hostEnv}"; physbox5_host_platform_from_uname Darwin; physbox5_host_platform_from_uname Linux; physbox5_host_platform_from_uname Plan9`);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "macos\nlinux\nunsupported\n");
});

test("graphical session import accepts only the declared environment keys", () => {
  const payload = [
    "DISPLAY=:9",
    "WAYLAND_DISPLAY=wayland-test",
    "XAUTHORITY=/tmp/test-auth",
    "XDG_RUNTIME_DIR=/tmp/test-runtime",
    "DBUS_SESSION_BUS_ADDRESS=unix:path=/tmp/test-bus",
    "IGNORED_SECRET=do-not-import",
  ].join("\\n");
  const result = bash(`
    source "${hostEnv}"
    unset DISPLAY WAYLAND_DISPLAY XAUTHORITY XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS IGNORED_SECRET
    physbox5_host_apply_environment $'${payload}'
    printf '%s|%s|%s|%s|%s|%s\\n' "$DISPLAY" "$WAYLAND_DISPLAY" "$XAUTHORITY" "$XDG_RUNTIME_DIR" "$DBUS_SESSION_BUS_ADDRESS" "\${IGNORED_SECRET:-missing}"
  `);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, ":9|wayland-test|/tmp/test-auth|/tmp/test-runtime|unix:path=/tmp/test-bus|missing\n");
});

test("setup reports the current supported platform without mutating the host", () => {
  const result = spawnSync("bash", [setup, "--print-platform"], { cwd: root, encoding: "utf8" });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /^(linux|macos)\n$/);
});

test("desktop automation can prefer X11 without changing macOS behavior", () => {
  const source = `DISPLAY=:9 WAYLAND_DISPLAY=wayland-test bash "${withHostEnv}" --linux-x11 bash -c 'printf "%s|%s\\n" "$DISPLAY" "\${WAYLAND_DISPLAY:-missing}"'`;
  const result = bash(source);
  assert.equal(result.status, 0, result.stderr);
  if (process.platform === "linux") assert.equal(result.stdout, ":9|missing\n");
  else assert.equal(result.stdout, ":9|wayland-test\n");
});

test("GNOME screen-lock replies map to an explicit host state", () => {
  const result = bash(`
    source "${hostEnv}"
    physbox5_host_parse_gnome_lock_state '(true,)'
    physbox5_host_parse_gnome_lock_state '(false,)'
    physbox5_host_parse_gnome_lock_state 'not-a-reply'
  `);
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "locked\nunlocked\nunavailable\n");
});

test("host setup invokes the conventional sibling hook with a bounded contract", () => {
  const temporary = mkdtempSync(join(tmpdir(), "physbox5-host-hook-"));
  try {
    const repo = join(temporary, "physbox5");
    const hook = join(temporary, "dotfiles", "scripts", "setup-physbox5-host.sh");
    mkdirSync(repo, { recursive: true });
    mkdirSync(resolve(hook, ".."), { recursive: true });
    writeFileSync(
      hook,
      "#!/usr/bin/env bash\nprintf 'hook=%s repo=%s active=%s\\n' \"$1\" \"$PHYSBOX5_REPO_ROOT\" \"$PHYSBOX5_HOST_SETUP_HOOK_ACTIVE\"\n",
    );
    chmodSync(hook, 0o755);

    const result = bash(`source "${hostEnv}"; physbox5_host_run_setup_hook "${repo}"`);
    assert.equal(result.status, 0, result.stderr);
    assert.match(result.stdout, /private machine setup hook:/);
    assert.match(result.stdout, /hook=--install .* active=1/);
    assert.match(result.stdout, new RegExp(`repo=${escapeRegExp(repo)}`));
  } finally {
    rmSync(temporary, { recursive: true, force: true });
  }
});

test("host setup hook can be disabled explicitly", () => {
  const result = spawnSync(
    "bash",
    ["-c", `source "${hostEnv}"; physbox5_host_run_setup_hook "${root}"`],
    {
      cwd: root,
      encoding: "utf8",
      env: { ...process.env, PHYSBOX5_HOST_SETUP_HOOK: "off" },
    },
  );
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout, "[host] private machine setup hook: none\n");
});

function bash(source) {
  return spawnSync("bash", ["-c", source], { cwd: root, encoding: "utf8" });
}

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}
