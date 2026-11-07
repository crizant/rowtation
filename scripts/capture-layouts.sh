#!/bin/sh
set -eu
if [ -n "${ROWTATION_BROWSER:-}" ]; then
  set -- --browser "$ROWTATION_BROWSER"
fi
mkdir -p docs/screenshots
nsh test App.tsx --target web "$@" < tests/capture-layouts.steps.js
