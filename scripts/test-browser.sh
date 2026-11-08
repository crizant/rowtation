#!/usr/bin/env bash
# Shared browser tests; *.headless.test.ts[x] stays in the default headless suite.
set -euo pipefail
cd "$(dirname "$0")/.."

set --
if [ -n "${ROWTATION_BROWSER:-}" ]; then
  set -- --browser "$ROWTATION_BROWSER"
fi

# Include new local tests as well as committed ones, respecting .gitignore.
# NUL delimiters preserve paths containing spaces. Fail on any test failure.
while IFS= read -r -d '' test_file; do
  nsh test "$test_file" --target web "$@"
done < <(git ls-files -z --cached --others --exclude-standard -- \
  '*.test.ts' '*.test.tsx' ':!:*.headless.test.ts' ':!:*.headless.test.tsx')
