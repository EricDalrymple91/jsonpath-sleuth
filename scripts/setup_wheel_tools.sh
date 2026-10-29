#!/usr/bin/env bash
# Uses a pinned prebuilt uv binary, not curl|sh, rustup, or public pip installs.
set -euo pipefail
: "${UV_INDEX_SOCKET_USERNAME:?Socket username is required}"
: "${UV_INDEX_SOCKET_PASSWORD:?Socket password is required}"
: "${RUNNER_TEMP:?Run on a GitHub-hosted runner}"

root=$(git rev-parse --show-toplevel)
image=ghcr.io/astral-sh/uv:0.13.6@sha256:af22a52f5eafef3d78e551599d405c02df61919d18028f0cea8eda93bc9986d0
mkdir -p "$RUNNER_TEMP/wheel-tools/bin"
container=$(docker create "$image" --version)
trap 'docker rm "$container" >/dev/null' EXIT
docker cp "$container:/uv" "$RUNNER_TEMP/wheel-tools/bin/uv"
uv="$RUNNER_TEMP/wheel-tools/bin/uv"
export UV_PYTHON_DOWNLOADS=never UV_NO_CACHE=true UV_NO_CONFIG=true
index=socket=https://registry.packages.sureapp.com/pypi/simple
"$uv" venv --python "$(command -v python)" "$RUNNER_TEMP/wheel-tools/venv"
"$uv" pip compile --python "$RUNNER_TEMP/wheel-tools/venv/bin/python" \
    --default-index "$index" --exclude-newer '7 days' --only-binary :all: \
    --generate-hashes "$root/scripts/wheel-tools.in" \
    --output-file "$RUNNER_TEMP/wheel-tools/requirements.txt"
"$uv" pip install --python "$RUNNER_TEMP/wheel-tools/venv/bin/python" \
    --default-index "$index" --exclude-newer '7 days' --only-binary :all: \
    --require-hashes --requirement "$RUNNER_TEMP/wheel-tools/requirements.txt"
echo "$RUNNER_TEMP/wheel-tools/venv/bin" >> "$GITHUB_PATH"
echo "$RUNNER_TEMP/wheel-tools/bin" >> "$GITHUB_PATH"
