#!/bin/sh
set -e

here="$(cd "$(dirname "$0")" && pwd)"
symbols="$(ls -d "$HOME"/.vscode/extensions/miguelsolorio.symbols-* 2>/dev/null | sort -V | tail -1)"

if [ -z "$symbols" ]; then
    echo "Symbols icon theme is not installed."
    exit 1
fi

cp "$here/icons/kp.svg" "$symbols/src/icons/files/khipu.svg"

python3 - "$symbols/src" <<'EOF'
import json
import os
import sys

src = sys.argv[1]
for name in ("symbol-icon-theme.json", "symbol-icon-theme.modified.json"):
    path = os.path.join(src, name)
    if not os.path.exists(path):
        continue
    with open(path) as f:
        theme = json.load(f)
    theme["iconDefinitions"]["khipu"] = {"iconPath": "./icons/files/khipu.svg"}
    theme.setdefault("fileExtensions", {})["kp"] = "khipu"
    with open(path, "w") as f:
        json.dump(theme, f, indent=2)
EOF

echo "Patched $(basename "$symbols"). Reload VS Code to see the KP icon."
