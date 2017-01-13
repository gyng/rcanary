#!/bin/bash
set -euxo pipefail

if [ "${1:-}" = "--" ]; then
  cargo run --release -- /app/config/config.toml | tee "/app/logs/`date +%s`.log"
else
  exec "$@"
fi
