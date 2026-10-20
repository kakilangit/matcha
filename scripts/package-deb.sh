#!/bin/sh
# Builds dist/matcha_<version>-1_<arch>.deb. Run on Linux from the repository root
# (usually via `make deb`). Installs cargo-deb with `cargo install` when missing.
#
# The package depends on the shared libraries of the build machine's glibc, so
# build on the oldest distribution you want to support.
set -eu

if [ "$(uname -s)" != Linux ]; then
  echo "package-deb.sh must run on Linux" >&2
  exit 1
fi

# cargo-deb needs dpkg-shlibdeps to work out the package's dependencies.
if ! command -v dpkg-shlibdeps >/dev/null; then
  echo "package-deb.sh needs dpkg-dev (e.g. apt install dpkg-dev)" >&2
  exit 1
fi

if ! cargo deb --version >/dev/null 2>&1; then
  cargo install cargo-deb --locked
fi

mkdir -p dist
cargo deb --locked --output dist/
