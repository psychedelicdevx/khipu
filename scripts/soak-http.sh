#!/bin/sh
set -e

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

PORT=${PORT:-8080}
export PORT
URL=${URL:-http://127.0.0.1:$PORT}
DURATION=${DURATION:-1h}
THREADS=${THREADS:-4}
CONNECTIONS=${CONNECTIONS:-100}
OUT=${OUT:-benchmarks/results/soak}
KHIPU=${KHIPU:-./target/release/khipu}

mkdir -p "$OUT"
cargo build --quiet --release

"$KHIPU" benchmarks/http/server.kp >/dev/null 2>&1 &
SERVER=$!
trap 'kill "$SERVER" 2>/dev/null || true' EXIT

for _ in $(seq 1 100); do
    if curl -fsS "$URL/hello" >/dev/null 2>&1; then break; fi
    sleep 0.05
done

RSS="$OUT/rss.log"
: > "$RSS"
printf 'epoch_s rss_kb\n' > "$RSS"

wrk -t"$THREADS" -c"$CONNECTIONS" -d"$DURATION" "$URL/hello" > "$OUT/wrk.txt" &
WRK=$!

while kill -0 "$WRK" 2>/dev/null; do
    if kill -0 "$SERVER" 2>/dev/null; then
        rss=$(ps -o rss= -p "$SERVER" | tr -d ' ')
        printf '%s %s\n' "$(date +%s)" "${rss:-0}" >> "$RSS"
    fi
    sleep 60
done
wait "$WRK" 2>/dev/null || true

echo "wrk output: $OUT/wrk.txt"
echo "rss samples: $RSS"
cat "$OUT/wrk.txt"
