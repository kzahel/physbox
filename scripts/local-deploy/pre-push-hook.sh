#!/usr/bin/env bash
set -euo pipefail

REMOTE="${1:-origin}"
TARGET_BRANCH="${PHYSBOX5_DEPLOY_AFTER_PUSH_BRANCH:-main}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ZERO_OID="0000000000000000000000000000000000000000"
while read -r _local_ref local_oid remote_ref _remote_oid; do
  if [ "$remote_ref" = "refs/heads/$TARGET_BRANCH" ] && [ "$local_oid" != "$ZERO_OID" ]; then
    "$SCRIPT_DIR/deploy-after-main-push.sh" --schedule \
      --remote "$REMOTE" --branch "$TARGET_BRANCH" --sha "$local_oid" || true
  fi
done
