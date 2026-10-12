#!/bin/sh
# Renders assets/icon.svg into the PNG and .icns files used for packaging.
# Requires rsvg-convert (librsvg) and python3. Run from the repository root.
set -eu

src=assets/icon.svg
out=assets/icons
mkdir -p "$out"

for size in 16 32 48 64 128 256 512 1024; do
  rsvg-convert --width "$size" --height "$size" "$src" --output "$out/matcha-$size.png"
done

# An .icns file is a header followed by PNG chunks, one per size and scale.
python3 - "$out" <<'EOF'
import struct
import sys
from pathlib import Path

out = Path(sys.argv[1])
chunks = [
    ("icp4", 16), ("icp5", 32), ("icp6", 64), ("ic07", 128), ("ic08", 256),
    ("ic09", 512), ("ic10", 1024), ("ic11", 32), ("ic12", 64), ("ic13", 256),
    ("ic14", 512),
]
body = b""
for kind, size in chunks:
    png = (out / f"matcha-{size}.png").read_bytes()
    body += kind.encode() + struct.pack(">I", len(png) + 8) + png
(out / "matcha.icns").write_bytes(b"icns" + struct.pack(">I", len(body) + 8) + body)
EOF

echo "Wrote $out"
