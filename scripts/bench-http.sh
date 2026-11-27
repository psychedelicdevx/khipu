#!/bin/sh
set -e

PORT=${PORT:-8080}
URL=${URL:-http://127.0.0.1:$PORT}
DURATION=${DURATION:-30s}
THREADS=${THREADS:-4}
CONNECTIONS=${CONNECTIONS:-100}

KHIPU=${KHIPU:-./target/release/khipu}
SERVER_PID=""

start_server() {
    "$KHIPU" benchmarks/http/server.kp &
    SERVER_PID=$!
    trap 'kill "$SERVER_PID" 2>/dev/null || true' EXIT
    for _ in $(seq 1 100); do
        if curl -fsS "$URL/hello" >/dev/null 2>&1; then
            return 0
        fi
        sleep 0.05
    done
    echo "server did not start" >&2
    exit 1
}

start_server

for route in hello json file; do
    echo "== wrk $URL/$route -t$THREADS -c$CONNECTIONS -d$DURATION"
    wrk -t"$THREADS" -c"$CONNECTIONS" -d"$DURATION" "$URL/$route"
done
