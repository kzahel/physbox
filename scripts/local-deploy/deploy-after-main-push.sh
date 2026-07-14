#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"
PROJECT_PARENT="$(dirname "$PROJECT_DIR")"
PROJECT_NAME="$(basename "$PROJECT_DIR")"
GIT_DIR="$(git -C "$PROJECT_DIR" rev-parse --git-dir)"
case "$GIT_DIR" in /*) ;; *) GIT_DIR="$PROJECT_DIR/$GIT_DIR" ;; esac
STATE_DIR="$GIT_DIR/physbox5-deploy"
DESIRED_FILE="$STATE_DIR/desired.tsv"
SUMMARY_FILE="$STATE_DIR/summary.tsv"
COMPLETED_FILE="$STATE_DIR/completed.tsv"
FAILED_FILE="$STATE_DIR/failed.tsv"
LOCK_DIR="$STATE_DIR/worker.lock"
LOG_FILE="$STATE_DIR/deploy.log"
DEPLOY_WORKTREE="${PHYSBOX5_DEPLOY_AFTER_PUSH_WORKTREE:-$PROJECT_PARENT/$PROJECT_NAME-deploy-worktree}"
DEFAULT_BRANCH="${PHYSBOX5_DEPLOY_AFTER_PUSH_BRANCH:-main}"
DEFAULT_REMOTE="${PHYSBOX5_DEPLOY_AFTER_PUSH_REMOTE:-origin}"
POLL_SECONDS="${PHYSBOX5_DEPLOY_AFTER_PUSH_POLL_SECONDS:-10}"
SETTLE_SECONDS="${PHYSBOX5_DEPLOY_AFTER_PUSH_SETTLE_SECONDS:-30}"
MAX_WAIT_SECONDS="${PHYSBOX5_DEPLOY_AFTER_PUSH_MAX_WAIT_SECONDS:-1800}"

timestamp() { date -u +%Y-%m-%dT%H:%M:%SZ; }
write_tsv() { local path="$1" tmp="$1.$$"; shift; (IFS=$'\t'; printf '%s\n' "$*") > "$tmp"; mv "$tmp" "$path"; }
summary() { mkdir -p "$STATE_DIR"; write_tsv "$SUMMARY_FILE" "$(timestamp)" "$1" "${2:-}" "${3:-}" "${4:-}" "${5:-}"; }
read_desired() { [ -f "$DESIRED_FILE" ] || return 1; IFS=$'\t' read -r DESIRED_SHA DESIRED_REMOTE DESIRED_BRANCH _ < "$DESIRED_FILE"; [ -n "$DESIRED_SHA" ]; }
desired_matches() { local sha="$1" remote="$2" branch="$3"; read_desired && [ "$DESIRED_SHA" = "$sha" ] && [ "$DESIRED_REMOTE" = "$remote" ] && [ "$DESIRED_BRANCH" = "$branch" ]; }
remote_sha() { git -C "$PROJECT_DIR" ls-remote --heads "$1" "$2" | awk -v ref="refs/heads/$2" '$2 == ref { print $1; exit }'; }
record_failure() { write_tsv "$FAILED_FILE" "$1" "$2" "$3" "$(timestamp)" "$4"; summary failed "$1" "$2" "$3" "$4"; }
acquire_lock() {
  mkdir -p "$STATE_DIR"
  if mkdir "$LOCK_DIR" 2>/dev/null; then printf '%s\n' "$$" > "$LOCK_DIR/pid"; trap 'rm -rf "$LOCK_DIR"' EXIT INT TERM; return; fi
  local pid=""; [ -f "$LOCK_DIR/pid" ] && pid="$(sed -n '1p' "$LOCK_DIR/pid")"
  if [ -n "$pid" ] && kill -0 "$pid" 2>/dev/null; then exit 0; fi
  rm -rf "$LOCK_DIR"; acquire_lock
}
prepare_worktree() {
  local sha="$1"
  if ! git -C "$DEPLOY_WORKTREE" rev-parse --is-inside-work-tree >/dev/null 2>&1; then
    [ ! -e "$DEPLOY_WORKTREE" ] || { echo "$DEPLOY_WORKTREE is not a Git worktree" >&2; return 1; }
    git -C "$PROJECT_DIR" worktree add --detach "$DEPLOY_WORKTREE" "$sha"
  fi
  git -C "$DEPLOY_WORKTREE" fetch --quiet "$DESIRED_REMOTE" "$DESIRED_BRANCH" || true
  git -C "$DEPLOY_WORKTREE" checkout --detach "$sha"
  git -C "$DEPLOY_WORKTREE" reset --hard "$sha"
  git -C "$DEPLOY_WORKTREE" clean -fd
  if [ -d "$PROJECT_DIR/node_modules" ] && [ ! -e "$DEPLOY_WORKTREE/node_modules" ]; then ln -s "$PROJECT_DIR/node_modules" "$DEPLOY_WORKTREE/node_modules"; fi
}
worker() {
  acquire_lock
  local deadline=$((SECONDS + MAX_WAIT_SECONDS)) last=""
  while read_desired; do
    local sha="$DESIRED_SHA" remote="$DESIRED_REMOTE" branch="$DESIRED_BRANCH"
    if [ "$sha" != "$last" ]; then last="$sha"; deadline=$((SECONDS + MAX_WAIT_SECONDS)); fi
    local current; current="$(remote_sha "$remote" "$branch" 2>/dev/null || true)"
    if [ "$current" != "$sha" ]; then
      if ((SECONDS >= deadline)); then record_failure "$sha" "$remote" "$branch" "remote timeout"; exit 1; fi
      summary waiting_remote "$sha" "$remote" "$branch" "remote is ${current:-unreadable}"; sleep "$POLL_SECONDS"; continue
    fi
    summary waiting_settle "$sha" "$remote" "$branch" "waiting ${SETTLE_SECONDS}s"
    local until=$((SECONDS + SETTLE_SECONDS))
    while ((SECONDS < until)); do sleep "$POLL_SECONDS"; desired_matches "$sha" "$remote" "$branch" || break; done
    desired_matches "$sha" "$remote" "$branch" || continue
    [ "$(remote_sha "$remote" "$branch" 2>/dev/null || true)" = "$sha" ] || continue
    summary preparing_worktree "$sha" "$remote" "$branch" "$DEPLOY_WORKTREE"
    if ! prepare_worktree "$sha"; then record_failure "$sha" "$remote" "$branch" "worktree failed"; exit 1; fi
    summary deploying "$sha" "$remote" "$branch" "npm run deploy"
    if (cd "$DEPLOY_WORKTREE" && npm run deploy); then
      write_tsv "$COMPLETED_FILE" "$sha" "$remote" "$branch" "$(timestamp)" "$DEPLOY_WORKTREE"
      summary succeeded "$sha" "$remote" "$branch" "exact SHA deployed"
      desired_matches "$sha" "$remote" "$branch" && rm -f "$DESIRED_FILE"
    else
      local status=$?; record_failure "$sha" "$remote" "$branch" "deploy exited $status"; exit "$status"
    fi
  done
  summary idle "" "" "" "no pending deploy"
}
schedule() {
  local remote="$DEFAULT_REMOTE" branch="$DEFAULT_BRANCH" sha=""
  while (($#)); do case "$1" in --remote) remote="$2"; shift 2;; --branch) branch="$2"; shift 2;; --sha) sha="$2"; shift 2;; *) echo "unknown argument: $1" >&2; exit 2;; esac; done
  [ -n "$sha" ] || { echo "--sha is required" >&2; exit 2; }
  mkdir -p "$STATE_DIR"; write_tsv "$DESIRED_FILE" "$sha" "$remote" "$branch" "$(timestamp)"; summary scheduled "$sha" "$remote" "$branch" "worker requested"
  nohup "$0" --worker >> "$LOG_FILE" 2>&1 </dev/null &
}
status() { echo "state dir: $STATE_DIR"; for file in summary desired completed failed; do local path="$STATE_DIR/$file.tsv"; [ -f "$path" ] && echo "$file: $(cat "$path")" || echo "$file: none"; done; echo "log: $LOG_FILE"; }

case "${1:-}" in --schedule) shift; schedule "$@";; --worker) worker;; --status) status;; *) echo "Usage: $0 --schedule --remote REMOTE --branch BRANCH --sha SHA | --worker | --status"; exit 2;; esac
