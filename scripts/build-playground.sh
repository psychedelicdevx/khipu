#!/bin/sh
set -e
cd "$(dirname "$0")/.."

cargo rustc --quiet --lib --release --target wasm32-unknown-unknown --crate-type cdylib
cp target/wasm32-unknown-unknown/release/khipu.wasm playground/khipu.wasm

python3 - <<'PY'
import json
from pathlib import Path

order = ["shapes", "types", "fizzbuzz", "primes", "functions", "lists", "records", "strings", "none", "testing"]
examples = [
    {"name": name, "source": Path(f"examples/{name}.kp").read_text()}
    for name in order
    if Path(f"examples/{name}.kp").exists()
]
Path("playground/examples.json").write_text(json.dumps(examples, indent=1) + "\n")
PY

echo "Built playground/. Serve it with: python3 -m http.server -d playground 8000"
