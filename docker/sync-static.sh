#!/bin/sh
set -eu

target="${1:-/shared-static}"

rm -rf "${target:?}"/* "${target}"/.[!.]* 2>/dev/null || true
cp -r /app/static/. "$target"
chmod -R a+rX "$target"