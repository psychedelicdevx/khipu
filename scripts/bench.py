import argparse
import datetime
import json
import os
import platform
import random
import re
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BENCHMARKS = ROOT / "benchmarks"
RESULTS = BENCHMARKS / "results"
README = ROOT / "README.md"
REPORT = ROOT / "BENCHMARKS.md"
KHIPU = ROOT / "target" / "release" / "khipu"

LANGUAGES = ["Khipu", "Python", "Node", "C", "Rust"]
EXTENSIONS = {"Khipu": "kp", "Python": "py", "Node": "js", "C": "c", "Rust": "rs"}
SIZE_LINE = {
    "kp": r"^(let N = )\d+",
    "py": r"^(N = )\d+",
    "js": r"^(const N = )\d+",
    "c": r"^(#define N )\d+",
    "rs": r"^(const N: i64 = )\d+",
}


def tool(*command):
    try:
        output = subprocess.run(command, capture_output=True, text=True).stdout.strip()
        return output.splitlines()[0] if output else "unknown"
    except OSError:
        return "not installed"


def environment():
    commit = tool("git", "-C", str(ROOT), "rev-parse", "--short", "HEAD")
    dirty = subprocess.run(["git", "-C", str(ROOT), "diff", "--quiet"]).returncode != 0
    return {
        "date": datetime.datetime.now().isoformat(timespec="seconds"),
        "khipu": commit + (" (uncommitted changes)" if dirty else ""),
        "cpu": tool("sysctl", "-n", "machdep.cpu.brand_string") or platform.processor(),
        "os": platform.platform(),
        "python": tool("python3", "--version"),
        "node": tool("node", "--version"),
        "c": tool("clang", "--version"),
        "rust": tool("rustc", "--version"),
    }


def available(language):
    command = {"Khipu": str(KHIPU), "Python": "python3", "Node": "node", "C": "clang", "Rust": "rustc"}[language]
    return Path(command).exists() if language == "Khipu" else shutil.which(command) is not None


def with_size(source, extension, size):
    if size is None:
        return source
    return re.sub(SIZE_LINE[extension], lambda m: m.group(1) + str(size), source, count=1, flags=re.M)


def prepare(benchmark, size, workdir):
    commands = {}
    for language in LANGUAGES:
        if not available(language):
            continue
        extension = EXTENSIONS[language]
        original = BENCHMARKS / f"{benchmark['path']}.{extension}"
        if not original.exists():
            continue
        source = workdir / f"{benchmark['name']}-{size}.{extension}"
        source.write_text(with_size(original.read_text(), extension, size))
        binary = workdir / f"{benchmark['name']}-{size}-{extension}"
        if language == "C":
            subprocess.run(["clang", "-O2", "-ffp-contract=off", "-o", str(binary), str(source), "-lm"], check=True)
            commands[language] = [str(binary)]
        elif language == "Rust":
            subprocess.run(["rustc", "--edition", "2021", "-C", "opt-level=3", "-o", str(binary), str(source)], check=True)
            commands[language] = [str(binary)]
        else:
            interpreter = {"Khipu": str(KHIPU), "Python": "python3", "Node": "node"}[language]
            commands[language] = [interpreter, str(source)]
    return commands


def run_once(command, workdir):
    output_path = workdir / "stdout.txt"
    error_path = workdir / "stderr.txt"
    with open(output_path, "w") as output, open(error_path, "w") as errors:
        start = time.perf_counter()
        process = subprocess.Popen(command, stdout=output, stderr=errors)
        _, status, usage = os.wait4(process.pid, 0)
        elapsed = time.perf_counter() - start
    process.returncode = os.waitstatus_to_exitcode(status)
    rss_bytes = usage.ru_maxrss if sys.platform == "darwin" else usage.ru_maxrss * 1024
    if process.returncode != 0:
        raise RuntimeError(error_path.read_text().strip() or f"exit code {process.returncode}")
    return elapsed * 1000, rss_bytes / (1024 * 1024), output_path.read_text().strip()


def measure(benchmark, mode, size, runs, workdir, shuffle):
    commands = prepare(benchmark, size, workdir)
    for command in commands.values():
        run_once(command, workdir)
    samples = {language: [] for language in commands}
    memory = {language: [] for language in commands}
    outputs = {}
    for _ in range(runs):
        order = list(commands)
        shuffle.shuffle(order)
        for language in order:
            elapsed, rss, output = run_once(commands[language], workdir)
            samples[language].append(elapsed)
            memory[language].append(rss)
            outputs.setdefault(language, output)
            if outputs[language] != output:
                raise RuntimeError(f"{benchmark['name']} ({language}) printed different answers between runs")
    if len(set(outputs.values())) > 1:
        raise RuntimeError(f"{benchmark['name']} {mode}: languages disagree: {outputs}")
    return {
        language: {
            "samples_ms": [round(value, 3) for value in samples[language]],
            "median_ms": round(statistics.median(samples[language]), 3),
            "min_ms": round(min(samples[language]), 3),
            "max_ms": round(max(samples[language]), 3),
            "peak_memory_mib": round(max(memory[language]), 2),
            "output": outputs[language],
        }
        for language in commands
    }


def ms(value):
    return f"{value:.1f} ms" if value < 10 else f"{value:.0f} ms"


def ratio(khipu, other):
    if khipu is None or other is None:
        return "–"
    if khipu <= other:
        return f"**{other / khipu:.1f}× faster**"
    return f"{khipu / other:.1f}× slower"


def table(results, mode, cell, extra=None):
    headers = ["Benchmark"] + LANGUAGES + (extra[0] if extra else [])
    lines = ["| " + " | ".join(headers) + " |", "|" + "---|" * len(headers)]
    for name, modes in results.items():
        if mode not in modes:
            continue
        row = modes[mode]
        cells = [cell(row[language]) if language in row else "–" for language in LANGUAGES]
        if extra:
            cells += extra[1](row)
        lines.append(f"| {name} | " + " | ".join(cells) + " |")
    return "\n".join(lines)


def versus(row):
    khipu = row.get("Khipu", {}).get("median_ms")
    return [ratio(khipu, row.get(language, {}).get("median_ms")) for language in ("Python", "Node")]


def wins(results, mode, language):
    rows = [modes[mode] for modes in results.values() if mode in modes and "Khipu" in modes[mode] and language in modes[mode]]
    won = sum(1 for row in rows if row["Khipu"]["median_ms"] <= row[language]["median_ms"])
    return won, len(rows)


def startup_line(results):
    row = results.get("empty", {}).get("cold", {})
    return " · ".join(f"{language} {ms(row[language]['median_ms'])}" for language in LANGUAGES if language in row)


def readme_section(results, meta, runs):
    sustained = {name: modes for name, modes in results.items() if "sustained" in modes}
    python_wins = wins(results, "sustained", "Python")
    node_wins = wins(results, "sustained", "Node")
    return f"""## Performance

Sustained runs use inputs large enough that start-up is a small share of the time. Median of {runs['sustained']} runs on {meta['cpu']}; lower is better.

{table(sustained, "sustained", lambda r: ms(r["median_ms"]), (["vs Python", "vs Node"], versus))}

Start-up, an empty program (median of {runs['cold']} runs): {startup_line(results)}.

On sustained runs Khipu beats Python on {python_wins[0]} of {python_wins[1]} benchmarks and Node on {node_wins[0]} of {node_wins[1]}. Cold-start results, spreads, memory use and the method are in [BENCHMARKS.md](BENCHMARKS.md).

"""


def report_section(results, meta, runs):
    spread = lambda r: f"{ms(r['median_ms'])} ({r['min_ms']:.0f}–{r['max_ms']:.0f})"
    memory = lambda r: f"{r['peak_memory_mib']:.1f} MiB"
    cold = {name: modes for name, modes in results.items() if name != "empty"}
    environment_lines = "\n".join(f"| {key} | {value} |" for key, value in meta.items())
    return f"""## Results

### Environment

| | |
|---|---|
{environment_lines}

### Start-up

An empty program, median of {runs['cold']} runs: {startup_line(results)}.

### Sustained

Median of {runs['sustained']} runs, with the fastest and slowest run in brackets.

{table(results, "sustained", spread, (["vs Python", "vs Node"], versus))}

### Cold start

Small inputs where start-up time matters. Median of {runs['cold']} runs.

{table(cold, "cold", spread, (["vs Python", "vs Node"], versus))}

### Peak memory

Highest resident memory of any sustained run (cold run for start-up).

{table({name: {"memory": modes.get("sustained", modes.get("cold"))} for name, modes in results.items()}, "memory", memory)}

Raw samples for every run are in [`benchmarks/results/latest.json`](benchmarks/results/latest.json).
"""


def replace_section(path, heading, content):
    text = path.read_text()
    pattern = rf"^{re.escape(heading)}\n.*?(?=^## |\Z)"
    if not re.search(pattern, text, flags=re.M | re.S):
        text = text.rstrip() + "\n\n" + content
    else:
        text = re.sub(pattern, lambda _: content, text, count=1, flags=re.M | re.S)
    path.write_text(text.rstrip() + "\n")


def main():
    parser = argparse.ArgumentParser(description="Run the Khipu benchmark suite")
    parser.add_argument("--quick", action="store_true", help="fewer runs, for a fast check")
    parser.add_argument("--only", help="comma-separated benchmark names")
    parser.add_argument("--cold-only", action="store_true", help="skip the sustained runs")
    parser.add_argument("--no-write", action="store_true", help="do not update README, BENCHMARKS.md or results")
    args = parser.parse_args()

    runs = {"cold": 5, "sustained": 3} if args.quick else {"cold": 21, "sustained": 5}
    suite = json.loads((BENCHMARKS / "suite.json").read_text())["benchmarks"]
    if args.only:
        wanted = set(args.only.split(","))
        suite = [b for b in suite if b["name"] in wanted]

    subprocess.run(["cargo", "build", "--release", "--quiet"], cwd=ROOT, check=True)
    meta = environment()
    shuffle = random.Random(20261126)
    results = {}

    with tempfile.TemporaryDirectory() as temp:
        workdir = Path(temp)
        for benchmark in suite:
            modes = [("cold", benchmark["cold"])]
            if benchmark["sustained"] is not None and not args.cold_only:
                modes.append(("sustained", benchmark["sustained"]))
            if benchmark["cold"] is None:
                modes = [("cold", None)]
            results[benchmark["name"]] = {}
            for mode, size in modes:
                result = measure(benchmark, mode, size, runs[mode], workdir, shuffle)
                results[benchmark["name"]][mode] = result
                summary = "  ".join(f"{language} {ms(r['median_ms'])}" for language, r in result.items())
                print(f"{benchmark['name']:<15} {mode:<9} {summary}", flush=True)

    if args.no_write:
        return

    RESULTS.mkdir(exist_ok=True)
    payload = json.dumps({"environment": meta, "runs": runs, "results": results}, indent=2) + "\n"
    (RESULTS / "latest.json").write_text(payload)
    stamp = datetime.date.today().isoformat() + "-" + meta["khipu"].split()[0]
    (RESULTS / f"{stamp}.json").write_text(payload)
    replace_section(README, "## Performance", readme_section(results, meta, runs))
    replace_section(REPORT, "## Results", report_section(results, meta, runs))


if __name__ == "__main__":
    main()
