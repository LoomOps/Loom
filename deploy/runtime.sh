#!/usr/bin/env bash
set -euo pipefail
export LOOM_INSTANCE=/loom/ops
export LOOM_PROJECT=loom
exec bash "$(dirname "$0")/compose.sh" "$@"
