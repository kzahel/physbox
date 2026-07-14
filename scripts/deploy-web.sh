#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BUCKET="${PHYSBOX5_DEPLOY_BUCKET:-physbox5}"
DIST_DIR="${PHYSBOX5_WEB_DIST_DIR:-$PROJECT_DIR/dist-web}"
WRANGLER=(npx --prefix "$PROJECT_DIR" wrangler)
DRY_RUN=0
if [ "${1:-}" = "--dry-run" ]; then DRY_RUN=1; shift; fi
if [ "$#" -ne 0 ]; then echo "Usage: $0 [--dry-run]" >&2; exit 2; fi

content_type() {
  case "$1" in
    *.html) echo "text/html; charset=utf-8" ;;
    *.js|*.mjs) echo "text/javascript; charset=utf-8" ;;
    *.wasm) echo "application/wasm" ;;
    *.json|*.map) echo "application/json" ;;
    *.css) echo "text/css; charset=utf-8" ;;
    *.png) echo "image/png" ;;
    *) echo "application/octet-stream" ;;
  esac
}

cd "$PROJECT_DIR"
npm run bundle:web
if [ "$DRY_RUN" -eq 1 ]; then
  rm -rf /tmp/physbox5-wrangler-dry-run
  (cd worker && "${WRANGLER[@]}" deploy --dry-run --outdir /tmp/physbox5-wrangler-dry-run)
  echo "PhysBox 5 deployment dry-run passed; no remote state changed"
  exit 0
fi

while IFS= read -r -d '' file; do
  key="${file#$DIST_DIR/}"
  "${WRANGLER[@]}" r2 object put "$BUCKET/$key" \
    --file="$file" --content-type="$(content_type "$file")" --remote >/dev/null
done < <(find "$DIST_DIR" -type f -print0)
(cd worker && "${WRANGLER[@]}" deploy)
echo "Deployed exact checkout to https://physbox5.kzahel.com/"
