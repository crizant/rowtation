#!/bin/sh
# Linux build used by Cloudflare Pages. Build command:
#   sh scripts/cloudflare-pages-build.sh
# Output directory: dist/Rowtation
set -eu

export PATH="$HOME/.cargo/bin:$HOME/.local/bin:$PATH"

if ! command -v rustup >/dev/null; then
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
if [ -f "$HOME/.cargo/env" ]; then
  # rustup's installer does not update PATH in the current shell.
  # shellcheck disable=SC1091
  . "$HOME/.cargo/env"
fi

rustup target add wasm32-unknown-unknown

mkdir -p "$HOME/.local/wasi-sdk"
curl -fsSL https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-34/wasi-sdk-34.0-x86_64-linux.tar.gz \
  | tar -xz --strip-components=1 -C "$HOME/.local/wasi-sdk"
export PATH="$HOME/.local/wasi-sdk/bin:$PATH"
export CC_wasm32_unknown_unknown="$HOME/.local/wasi-sdk/bin/clang"
export AR_wasm32_unknown_unknown="$HOME/.local/wasi-sdk/bin/llvm-ar"
clang --version
llvm-ar --version

export NSH_VERSION=v0.5.0
curl -fsSL "https://github.com/longbridge/nsh/releases/download/${NSH_VERSION}/install.sh" | sh

# Browser file APIs need the wasm-bindgen CLI that matches Cargo.lock.
bindgen_version=$(awk '
  $0 == "name = \"wasm-bindgen\"" { found = 1; next }
  found && $1 == "version" { gsub(/"/, "", $3); print $3; exit }
' Cargo.lock)
if [ -z "$bindgen_version" ]; then
  echo "wasm-bindgen version missing from Cargo.lock" >&2
  exit 1
fi
cargo install wasm-bindgen-cli --version "$bindgen_version" --locked

nsh package --target web -o dist
node scripts/web-metadata.mjs dist/Rowtation/index.html
node scripts/web-metadata.mjs --check dist/Rowtation/index.html
