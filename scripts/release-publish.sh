#!/bin/sh
# Creates the GitHub release for a tag from the packages in dist/, using the
# version's CHANGELOG.md section as the notes. Needs the GitHub CLI (`gh`) with
# permission to create releases (GH_TOKEN in CI). Run from the repository root.
set -eu

tag=${1:?usage: release-publish.sh <tag>}
version=${tag#v}

notes=$(mktemp)
trap 'rm -f "$notes"' EXIT
./scripts/changelog-notes.sh "$version" > "$notes"

set -- dist/*
if [ ! -e "$1" ]; then
  echo "No packages in dist/ to publish" >&2
  exit 1
fi

case $version in
  *-*) set -- "$@" --prerelease ;;
esac

gh release create "$tag" "$@" \
  --verify-tag \
  --title "Matcha $version" \
  --notes-file "$notes"
