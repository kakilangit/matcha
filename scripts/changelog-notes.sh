#!/bin/sh
# Prints the CHANGELOG.md section for a version (without its heading), for use as
# release notes. Run from the repository root.
set -eu

version=${1:?usage: changelog-notes.sh <version>}

# Collect lines from the version's heading up to the next version heading or the
# link definitions at the bottom, then print them without surrounding blank lines.
awk -v heading="## [$version]" '
  index($0, heading) == 1 { found = 1; next }
  found && (/^## \[/ || /^\[[^]]+\]: /) { exit }
  found { lines[++n] = $0 }
  END {
    first = 1
    while (first <= n && lines[first] ~ /^[[:space:]]*$/) first++
    last = n
    while (last >= first && lines[last] ~ /^[[:space:]]*$/) last--
    for (i = first; i <= last; i++) print lines[i]
  }
' CHANGELOG.md
