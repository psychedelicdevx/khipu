import os
import re
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KHIPU = ROOT / "target" / "release" / "khipu"
README = ROOT / "README.md"
TIMEOUT = 20


def free_port():
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def snippets():
    text = README.read_text()
    found = []
    for match in re.finditer(r"^```(\w*)\n(.*?)^```", text, flags=re.M | re.S):
        lang, body = match.group(1), match.group(2)
        line = text[: match.start()].count("\n") + 2
        runnable = lang == "js" or (lang == "" and re.match(r"^(fn|let|struct|enum) ", body))
        if runnable:
            found.append((line, body))
    return found


def run(args, cwd, stdin_text="", timeout=TIMEOUT):
    return subprocess.run(
        [str(KHIPU), *args],
        cwd=cwd,
        input=stdin_text,
        capture_output=True,
        text=True,
        timeout=timeout,
    )


def wait_for_server(process, port):
    deadline = time.time() + 10
    while time.time() < deadline:
        if process.poll() is not None:
            return False
        try:
            with socket.create_connection(("127.0.0.1", port), timeout=0.2):
                return True
        except OSError:
            time.sleep(0.05)
    return False


class Checker:
    def __init__(self):
        self.failures = []
        self.checked = 0
        self.servers = []
        self.port = free_port()

    def fail(self, what, detail):
        self.failures.append(f"{what}\n{detail.rstrip()}")

    def ok(self, what):
        self.checked += 1
        print(f"  ok  {what}")

    def program(self, what, source, cwd):
        source = source.replace("8080", str(self.port))
        path = Path(cwd) / "main.kp"
        path.write_text(source)
        if "Http.serve(" in source:
            process = subprocess.Popen(
                [str(KHIPU), str(path)], cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True
            )
            self.servers.append(process)
            if wait_for_server(process, self.port):
                return self.ok(f"{what} (serving)")
            _, errors = process.communicate(timeout=5)
            return self.fail(what, errors or "the server did not start")
        if 'test "' in source:
            result = run(["test", str(path)], cwd)
        else:
            result = run([str(path)], cwd)
        if result.returncode != 0:
            return self.fail(what, result.stderr or result.stdout)
        self.ok(what)

    def readme(self):
        with tempfile.TemporaryDirectory() as temp:
            project = Path(temp) / "project"
            colors = Path(temp) / "colors"
            project.mkdir()
            colors.mkdir()
            (colors / "main.kp").write_text(
                'fn red(text: string) -> string {\n    return "<red>${text}</red>"\n}\n'
            )
            (project / "khipu.json").write_text('{ "name": "project", "dependencies": { "colors": "path:../colors" } }\n')
            install = run(["install"], project)
            if install.returncode != 0:
                self.fail("README: installing the colors package for the import snippet", install.stderr)
            for line, body in snippets():
                self.program(f"README.md snippet at line {line}", body, project)
            self.stop_servers()

    def examples(self):
        for path in sorted(ROOT.glob("examples/**/*.kp")):
            if path.parent.name == "modules" and path.name != "main.kp":
                continue
            source = path.read_text()
            with tempfile.TemporaryDirectory() as temp:
                for sibling in path.parent.glob("*.kp"):
                    (Path(temp) / sibling.name).write_text(sibling.read_text().replace("8080", str(self.port)))
                if "Http.get(" in source or "Http.post(" in source:
                    server = path.parent / "server.kp"
                    if server.exists():
                        self.program(f"{server.relative_to(ROOT)}", server.read_text(), temp)
                self.program(str(path.relative_to(ROOT)), source, temp)
                self.stop_servers()

    def stop_servers(self):
        for process in self.servers:
            process.kill()
            process.wait()
        self.servers.clear()
        self.port = free_port()


def main():
    if not KHIPU.exists():
        print("build the release binary first: cargo build --release", file=sys.stderr)
        return 2
    checker = Checker()
    checker.readme()
    checker.examples()
    print(f"\n{checker.checked} snippets and examples ran")
    if checker.failures:
        print(f"{len(checker.failures)} failed:\n", file=sys.stderr)
        for failure in checker.failures:
            print(failure + "\n", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
