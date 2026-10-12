#!/bin/sh
# Checks that a release tag (e.g. v0.1.0) can be published: it matches the version
# in Cargo.toml, CHANGELOG.md has a section for it, and its commit (or HEAD, if the
# tag doesn't exist yet) is on origin/main. Run from the repository root.
set -eu

tag=${1:?usage: release-check.sh <tag>}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -n 1)

if [ "$tag" != "v$version" ]; then
  echo "Tag $tag does not match Cargo.toml version $version (expected v$version)" >&2
  exit 1
fi

if ! grep -q "^## \[$version\]" CHANGELOG.md; then
  echo "CHANGELOG.md has no '## [$version]' section" >&2
  exit 1
fi

if ! git rev-parse --verify --quiet origin/main >/dev/null; then
  echo "origin/main is not available; fetch it first (git fetch origin main)" >&2
  exit 1
fi
commit=$(git rev-parse --verify --quiet "refs/tags/$tag^{commit}" || git rev-parse HEAD)
if ! git merge-base --is-ancestor "$commit" origin/main; then
  echo "$tag ($commit) is not on origin/main" >&2
  exit 1
fi

echo "Release $tag matches Cargo.toml and CHANGELOG.md, and is on main"
