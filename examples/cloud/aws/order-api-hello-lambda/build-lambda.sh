#!/usr/bin/env bash
# Bundle kernlc + kn sources + Rust bootstrap into ./dist for CDK asset packaging.
# Cross-compiles for Lambda provided.al2023 (Linux glibc x86_64) using cargo-zigbuild (no Docker).
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
# kernl repo root: …/kernl
KERNL_ROOT="$(cd "${SCRIPT_DIR}/../../../.." && pwd)"
TARGET="${LAMBDA_TARGET:-x86_64-unknown-linux-gnu}"
DIST="${SCRIPT_DIR}/dist"

if ! command -v cargo >/dev/null 2>&1; then
  echo "Rust toolchain (cargo) not found." >&2
  exit 1
fi

if ! cargo zigbuild --help >/dev/null 2>&1; then
  echo "Missing cargo-zigbuild. Install with:" >&2
  echo "  cargo install cargo-zigbuild" >&2
  echo "and install Zig (https://ziglang.org/). See README.md." >&2
  exit 1
fi

rustup target add "${TARGET}" >/dev/null 2>&1 || true

echo "==> Building kernlc (${TARGET})"
(cd "${KERNL_ROOT}/compiler" && cargo zigbuild --release --target "${TARGET}")

echo "==> Building Lambda bootstrap (${TARGET})"
(cd "${SCRIPT_DIR}/lambda-bootstrap" && cargo zigbuild --release --target "${TARGET}")

echo "==> Staging dist/"
rm -rf "${DIST}"
mkdir -p "${DIST}/kn"
cp "${KERNL_ROOT}/compiler/target/${TARGET}/release/kernlc" "${DIST}/kernlc"
cp "${SCRIPT_DIR}/lambda-bootstrap/target/${TARGET}/release/bootstrap" "${DIST}/bootstrap"
cp "${SCRIPT_DIR}/kn/order_api.knl" "${DIST}/kn/order_api.knl"
chmod +x "${DIST}/kernlc" "${DIST}/bootstrap"

echo "==> Done: ${DIST}/ (use with CDK fromAsset)"
