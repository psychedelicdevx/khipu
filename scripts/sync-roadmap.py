import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ROADMAP = ROOT / "ROADMAP.md"
TARGETS = [ROADMAP, ROOT / "README.md"]
BAR_WIDTH = 10


def read_phases(text):
    phases = []
    for block in re.split(r"^## ", text, flags=re.M)[1:]:
        heading = block.splitlines()[0]
        match = re.match(r"Phase (\d+): (.+?)( ✅)?$", heading)
        if not match:
            continue
        items = re.findall(r"^- \[( |x)\] (.+)$", block, flags=re.M)
        phases.append(
            {
                "number": match.group(1),
                "title": match.group(2),
                "done": sum(1 for mark, _ in items if mark == "x"),
                "total": len(items),
                "next": next((name for mark, name in items if mark == " "), None),
            }
        )
    return phases


def bar(done, total):
    filled = round(done / total * BAR_WIDTH) if total else 0
    if done and not filled:
        filled = 1
    return "█" * filled + "░" * (BAR_WIDTH - filled)


def short_title(title):
    return re.sub(r"\s*\(.*\)$", "", title)


def progress_table(phases):
    rows = ["| Phase | Progress |", "|---|---|"]
    for phase in phases:
        rows.append(
            f"| {phase['number']}. {short_title(phase['title'])} "
            f"| `{bar(phase['done'], phase['total'])}` {phase['done']} / {phase['total']} |"
        )
    return "\n".join(rows)


def next_up(phases):
    for phase in phases:
        if phase["next"]:
            return f"**Next up:** {phase['next']}"
    return "**Next up:** everything is done 🎉"


def mark_complete_phases(text, phases):
    for phase in phases:
        heading = f"## Phase {phase['number']}: {phase['title']}"
        complete = phase["total"] and phase["done"] == phase["total"]
        text = re.sub(
            rf"^{re.escape(heading)}( ✅)?$",
            lambda _: heading + (" ✅" if complete else ""),
            text,
            flags=re.M,
        )
    return text


def main():
    phases = read_phases(ROADMAP.read_text())
    table = progress_table(phases)
    upcoming = next_up(phases)

    for path in TARGETS:
        if not path.exists():
            continue
        text = path.read_text()
        text = re.sub(
            r"^\| Phase \| Progress \|\n(?:\|.*\|\n?)+",
            lambda _: table + "\n",
            text,
            flags=re.M,
        )
        text = re.sub(r"^\*\*Next up:\*\*.*$", lambda _: upcoming, text, flags=re.M)
        if path == ROADMAP:
            text = mark_complete_phases(text, phases)
        path.write_text(text)

    total_done = sum(phase["done"] for phase in phases)
    total = sum(phase["total"] for phase in phases)
    print(f"Roadmap: {total_done} / {total} done. {upcoming.replace('**', '')}")


if __name__ == "__main__":
    main()
