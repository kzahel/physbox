#!/usr/bin/env bash
set -euo pipefail

PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"
git -C "$PROJECT_DIR" config core.hooksPath .githooks
chmod +x "$PROJECT_DIR/.githooks/pre-push" \
  "$PROJECT_DIR/scripts/deploy-web.sh" \
  "$PROJECT_DIR/scripts/local-deploy/"*.sh
echo "PhysBox 5 Git hooks installed: core.hooksPath=.githooks"
