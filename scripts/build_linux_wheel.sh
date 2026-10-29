#!/usr/bin/env bash
set -euo pipefail
: "${UV_INDEX_SOCKET_USERNAME:?Socket username is required}"
: "${UV_INDEX_SOCKET_PASSWORD:?Socket password is required}"
: "${RUNNER_TEMP:?Run on a GitHub-hosted runner}"
: "${GITHUB_SHA:?Source commit is required}"

# Take the installed compiler binaries directly, not rustup shims. No bootstrap.
toolchain=$(python - <<'PY'
from pathlib import Path
import os
paths = sorted((Path.home() / '.rustup/toolchains').glob('stable-*/bin/rustc'))
if len(paths) != 1 or not paths[0].with_name('cargo').is_file():
    raise SystemExit('An existing stable Rust toolchain is required; refusing to download one')
print(paths[0].parent)
PY
)
export PATH="$toolchain:$PATH" RUSTC="$toolchain/rustc" RUSTDOC="$toolchain/rustdoc"
rustc --version
cargo --version

home=$(mktemp -d "$RUNNER_TEMP/socket-cargo.XXXXXX")
trap 'rm -f "$home/config.toml"' EXIT
export CARGO_HOME="$home"
cat > "$CARGO_HOME/config.toml" <<'TOML'
[source.crates-io]
replace-with = "socket"
[source.socket]
registry = "sparse+https://registry.packages.sureapp.com/cargo/"
[registries.socket]
index = "sparse+https://registry.packages.sureapp.com/cargo/"
credential-provider = "cargo:token"
TOML
export CARGO_REGISTRIES_SOCKET_TOKEN
CARGO_REGISTRIES_SOCKET_TOKEN=$(python - <<'PY'
import base64, os
value = os.environ['UV_INDEX_SOCKET_USERNAME'] + ':' + os.environ['UV_INDEX_SOCKET_PASSWORD']
print('Basic ' + base64.b64encode(value.encode()).decode())
PY
)
printf '::add-mask::%s\n' "$CARGO_REGISTRIES_SOCKET_TOKEN"

# Project-local configuration can override CARGO_HOME. This repo needs none.
if find . -path '*/.cargo/config' -o -path '*/.cargo/config.toml' | grep -q .; then
    echo 'Refusing a project Cargo config that could override Socket' >&2
    exit 1
fi
cargo fetch --locked
export CARGO_NET_OFFLINE=true
cargo test --locked --offline --no-default-features
# Direct CLI + an existing compiler avoids Python's maturin/puccinialin bootstrap.
maturin build --release --locked --offline --compatibility manylinux_2_39 --out dist
git diff --exit-code -- Cargo.lock Cargo.toml pyproject.toml src
python scripts/wheel_release.py manifest --architecture "$(uname -m)" --commit "$GITHUB_SHA"
rustc -vV > "dist/toolchain-$(uname -m).txt"
cp "$RUNNER_TEMP/wheel-tools/requirements.txt" "dist/tools-$(uname -m).txt"
