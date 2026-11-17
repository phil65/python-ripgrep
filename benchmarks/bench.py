"""Reference benchmarks for ripgrep_rs.

Times every public entry point on one or more corpora and, where an
equivalent exists, the `rg` CLI as a baseline. Results can be saved as JSON
and compared against a previous run.

    python benchmarks/bench.py                      # pinned CPython corpus
    python benchmarks/bench.py --corpus ~/src/linux --save after.json
    python benchmarks/bench.py --compare before.json

Run from outside the repository root (or with `python -P`) so the local
`ripgrep_rs/` source directory doesn't shadow the installed extension.
"""

from __future__ import annotations

import argparse
import importlib.metadata
import json
import platform
import shutil
import statistics
import subprocess
import time
from collections.abc import Callable
from pathlib import Path
from typing import Any

import ripgrep_rs as rg

CACHE_DIR = Path.home() / ".cache" / "ripgrep-rs-bench"
CPYTHON_TAG = "v3.13.0"
RARE = "Xyzzy" + "NeverAppears42"  # split so this file never matches
COMMON = "import"


def default_corpus() -> Path:
    path = CACHE_DIR / f"cpython-{CPYTHON_TAG}"
    if not path.exists():
        CACHE_DIR.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            [
                "git",
                "clone",
                "--quiet",
                "--depth=1",
                f"--branch={CPYTHON_TAG}",
                "https://github.com/python/cpython.git",
                str(path),
            ],
            check=True,
        )
    return path


def count_lines(result: list[str]) -> int:
    return sum(chunk.count("\n") for chunk in result)


def count_json_matches(result: list[str]) -> int:
    return sum(chunk.count('"type":"match"') for chunk in result)


def cases(
    root: str,
) -> list[tuple[str, Callable[[], Any], Callable[[Any], int], list[str] | None]]:
    """(name, library call, result counter, equivalent rg CLI args)."""
    sort_path = rg.PySortMode(rg.PySortModeKind.Path)
    return [
        ("files", lambda: rg.files(paths=[root]), len, ["--files", root]),
        (
            "files sorted",
            lambda: rg.files(paths=[root], sort=sort_path),
            len,
            ["--files", "--sort=path", root],
        ),
        ("files absolute", lambda: rg.files(paths=[root], absolute=True), len, None),
        ("files_with_info", lambda: rg.files_with_info(paths=[root]), len, None),
        (
            "search rare",
            lambda: rg.search([RARE], paths=[root]),
            count_lines,
            ["--no-heading", RARE, root],
        ),
        (
            "search common",
            lambda: rg.search([COMMON], paths=[root], line_number=True),
            count_lines,
            ["--no-heading", "-n", COMMON, root],
        ),
        (
            "search common sorted",
            lambda: rg.search([COMMON], paths=[root], line_number=True, sort=sort_path),
            count_lines,
            ["--no-heading", "-n", "--sort=path", COMMON, root],
        ),
        (
            "search max_count=1",
            lambda: rg.search([COMMON], paths=[root], max_count=1),
            count_lines,
            ["--no-heading", "-m1", COMMON, root],
        ),
        (
            "search json",
            lambda: rg.search([COMMON], paths=[root], json=True),
            count_json_matches,
            ["--json", COMMON, root],
        ),
        (
            "structured rare",
            lambda: rg.search_structured([RARE], paths=[root]),
            len,
            None,
        ),
        (
            "structured common",
            lambda: rg.search_structured([COMMON], paths=[root]),
            len,
            None,
        ),
        (
            "structured max_total=10",
            lambda: rg.search_structured([COMMON], paths=[root], max_total=10),
            len,
            None,
        ),
    ]


def run_cli(args: list[str]) -> bytes:
    return subprocess.run(["rg", *args], capture_output=True, check=False).stdout


def cli_count(args: list[str], out: bytes) -> int:
    if "--json" in args:
        return out.count(b'"type":"match"')
    return out.count(b"\n")


def measure(fn: Callable[[], Any], repeat: int) -> tuple[float, float, Any]:
    """Return (median ms, min ms, last result) after one warmup call."""
    result = fn()
    times = []
    for _ in range(repeat):
        start = time.perf_counter()
        result = fn()
        times.append((time.perf_counter() - start) * 1000)
    return statistics.median(times), min(times), result


def bench(corpus: Path, repeat: int, with_cli: bool) -> list[dict[str, Any]]:
    rows = []
    for name, fn, counter, cli_args in cases(str(corpus)):
        median, best, result = measure(fn, repeat)
        row: dict[str, Any] = {
            "case": name,
            "median_ms": median,
            "min_ms": best,
            "n": counter(result),
        }
        if with_cli and cli_args is not None:
            cmedian, cbest, out = measure(lambda a=cli_args: run_cli(a), repeat)
            row |= {
                "rg_median_ms": cmedian,
                "rg_min_ms": cbest,
                "rg_n": cli_count(cli_args, out),
            }
        rows.append(row)
    return rows


def fmt(value: float | None, spec: str = "9.1f") -> str:
    return f"{value:{spec}}" if value is not None else f"{'-':>9}"


def print_table(
    corpus: str, rows: list[dict[str, Any]], baseline: dict[str, dict] | None
) -> None:
    print(f"\n{corpus}")
    header = f"{'case':26} {'median ms':>9} {'n':>8} {'rg ms':>9} {'rg n':>8}"
    if baseline is not None:
        header += f" {'base ms':>9} {'speedup':>8}"
    print(header)
    for r in rows:
        line = (
            f"{r['case']:26} {fmt(r['median_ms'])} {r['n']:>8} "
            f"{fmt(r.get('rg_median_ms'))} {r.get('rg_n', '-'):>8}"
        )
        if baseline is not None:
            base = baseline.get(r["case"])
            if base:
                line += f" {fmt(base['median_ms'])} {base['median_ms'] / r['median_ms']:7.2f}x"
                if base["n"] != r["n"]:
                    line += f"  (n was {base['n']})"
        print(line)


def main() -> None:
    parser = argparse.ArgumentParser(
        description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter
    )
    parser.add_argument(
        "--corpus", action="append", type=Path, help="directory to search (repeatable)"
    )
    parser.add_argument("--repeat", type=int, default=7)
    parser.add_argument("--no-cli", action="store_true", help="skip rg CLI baseline")
    parser.add_argument("--save", type=Path, help="write results as JSON")
    parser.add_argument("--compare", type=Path, help="JSON from a previous --save")
    args = parser.parse_args()

    corpora = [p.expanduser().resolve() for p in args.corpus or [default_corpus()]]
    with_cli = not args.no_cli and shutil.which("rg") is not None
    baseline = json.loads(args.compare.read_text())["corpora"] if args.compare else None

    report: dict[str, Any] = {
        "ripgrep_rs": importlib.metadata.version("ripgrep-rs"),
        "python": platform.python_version(),
        "machine": platform.machine(),
        "repeat": args.repeat,
        "corpora": {},
    }
    if with_cli:
        report["rg"] = run_cli(["--version"]).decode().splitlines()[0]
    for corpus in corpora:
        rows = bench(corpus, args.repeat, with_cli)
        report["corpora"][str(corpus)] = {r["case"]: r for r in rows}
        base = None
        if baseline is not None:
            base = baseline.get(str(corpus), {})
        print_table(str(corpus), rows, base)

    if args.save:
        args.save.write_text(json.dumps(report, indent=2))


if __name__ == "__main__":
    main()
