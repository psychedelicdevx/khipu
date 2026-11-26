import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCHMARKS = ROOT / "benchmarks"
README = ROOT / "README.md"
KHIPU = ROOT / "target" / "release" / "khipu"
RUNS = 5

LANGUAGES = [
    ("Khipu", [str(KHIPU)], ".kp"),
    ("Python", ["python3"], ".py"),
    ("Node", ["node"], ".js"),
]


def timed(command):
    best = None
    output = None
    for _ in range(RUNS):
        start = time.perf_counter()
        result = subprocess.run(command, capture_output=True, text=True)
        elapsed = time.perf_counter() - start
        if result.returncode != 0:
            return None, result.stderr.strip() or result.stdout.strip()
        output = result.stdout.strip()
        best = elapsed if best is None else min(best, elapsed)
    return best, output


def compare(khipu, other):
    if khipu is None or other is None:
        return "–"
    if khipu <= other:
        return f"**{other / khipu:.1f}× faster**"
    return f"{khipu / other:.1f}× slower"


def cpu_name():
    try:
        return subprocess.run(
            ["sysctl", "-n", "machdep.cpu.brand_string"], capture_output=True, text=True
        ).stdout.strip()
    except OSError:
        return ""


def main():
    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=ROOT, check=True)

    rows = []
    failures = []
    for script in sorted(BENCHMARKS.glob("*.kp")):
        name = script.stem
        times = {}
        outputs = {}
        for language, command, extension in LANGUAGES:
            source = script.with_suffix(extension)
            if not source.exists() or not shutil.which(command[0]):
                continue
            elapsed, output = timed(command + [str(source)])
            if elapsed is None:
                failures.append(f"{name} ({language}): {output}")
                continue
            times[language] = elapsed
            outputs[language] = output

        answers = set(outputs.values())
        if len(answers) > 1:
            failures.append(f"{name}: different answers {outputs}")
            continue

        cell = lambda language: f"{times[language] * 1000:.0f} ms" if language in times else "–"
        rows.append(
            f"| {name} | {cell('Khipu')} | {cell('Python')} | {cell('Node')} "
            f"| {compare(times.get('Khipu'), times.get('Python'))} "
            f"| {compare(times.get('Khipu'), times.get('Node'))} |"
        )
        print(rows[-1])

    for failure in failures:
        print("FAILED:", failure, file=sys.stderr)
    if failures:
        sys.exit(1)

    table = "\n".join(
        ["| Benchmark | Khipu | Python | Node | vs Python | vs Node |", "|---|---|---|---|---|---|"] + rows
    )
    note = f"Best of {RUNS} runs, including start-up, on {cpu_name() or 'this machine'}. Run `python3 scripts/bench.py` to measure again."

    text = README.read_text()
    text = re.sub(
        r"^\| Benchmark \| Khipu \|.*\n(?:\|.*\|\n?)+",
        lambda _: table + "\n",
        text,
        flags=re.M,
    )
    text = re.sub(r"^Best of \d+ runs, including start-up.*$", lambda _: note, text, flags=re.M)
    README.write_text(text)


if __name__ == "__main__":
    main()
