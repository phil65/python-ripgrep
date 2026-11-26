"""Tests for search() and search_structured()."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

from ripgrep_rs import files, search, search_structured


@pytest.fixture
def tree(tmp_path: Path) -> Path:
    (tmp_path / "a.txt").write_bytes(b"foo 1\nfoo 2\nfoo 3\nfoo 4\n")
    (tmp_path / "b.txt").write_bytes(b"bar\nfoo 5\n")
    return tmp_path


@pytest.fixture
def tree_with_binary(tree: Path) -> Path:
    (tree / "blob.bin").write_bytes(b"\x00\x01foo binary\n")
    return tree


def test_search_structured_skips_binary_files(tree_with_binary: Path):
    matches = search_structured(["foo"], paths=[str(tree_with_binary)])
    assert "blob.bin" not in {Path(m.path).name for m in matches}


def test_search_json_skips_binary_files(tree_with_binary: Path):
    result = search(["foo"], paths=[str(tree_with_binary)], json=True)
    assert not any("blob.bin" in chunk for chunk in result)


def test_search_skips_binary_files(tree_with_binary: Path):
    result = search(["foo"], paths=[str(tree_with_binary)])
    assert not any("blob.bin" in chunk for chunk in result)


def test_search_max_count(tree: Path):
    result = search(["foo"], paths=[str(tree / "a.txt")], max_count=1)
    assert result == ["foo 1\n"]


def test_search_structured_max_count(tree: Path):
    matches = search_structured(["foo"], paths=[str(tree)], max_count=1)
    per_file = {Path(m.path).name: m.line_number for m in matches}
    assert per_file == {"a.txt": 1, "b.txt": 2}
    assert len(matches) == 2


@pytest.mark.skipif(sys.platform == "win32", reason="needs a pty")
def test_search_output_independent_of_tty(tree: Path):
    """Default heading/line_number must not depend on whether stdout is a tty."""
    import pty

    code = (
        "import sys, ripgrep_rs; "
        f"r = ripgrep_rs.search(['foo'], paths=[{str(tree / 'b.txt')!r}]); "
        "sys.stderr.write(repr(r))"
    )
    _, slave = pty.openpty()
    try:
        proc = subprocess.run(
            [sys.executable, "-c", code],
            stdout=slave,
            stderr=subprocess.PIPE,
            stdin=subprocess.DEVNULL,
            text=True,
            check=True,
        )
    finally:
        os.close(slave)
    assert proc.stderr == repr(["foo 5\n"])


def test_search_without_paths_ignores_stdin(tree: Path):
    """Without paths, search the cwd even when stdin is a readable pipe."""
    code = "import ripgrep_rs; print(sorted(ripgrep_rs.search(['foo'])))"
    proc = subprocess.run(
        [sys.executable, "-c", code],
        input="foo from stdin\n",
        capture_output=True,
        text=True,
        cwd=tree,
        timeout=30,
        check=True,
    )
    assert "stdin" not in proc.stdout
    assert "foo 5" in proc.stdout


@pytest.fixture
def mixed_tree(tmp_path: Path) -> Path:
    (tmp_path / "a.py").write_bytes(b"hit\n")
    (tmp_path / "b.toml").write_bytes(b"hit\n")
    (tmp_path / "c.txt").write_bytes(b"x" * 50 + b"\nhit late\n")
    return tmp_path


def _names(matches) -> list[str]:
    return sorted(Path(m.path).name for m in matches)


def test_glob_not_excludes(mixed_tree: Path):
    matches = search_structured(["hit"], paths=[str(mixed_tree)], glob_not=["*.toml"])
    assert _names(matches) == ["a.py", "c.txt"]


def test_glob_not_wins_over_globs(mixed_tree: Path):
    matches = search_structured(
        ["hit"], paths=[str(mixed_tree)], globs=["*.py", "*.toml"], glob_not=["*.toml"]
    )
    assert _names(matches) == ["a.py"]


def test_glob_not_leading_bang_unnegates(mixed_tree: Path):
    # Like `rg -G '!*.py'`: the double negation is a whitelist glob.
    matches = search_structured(["hit"], paths=[str(mixed_tree)], glob_not=["!*.py"])
    assert _names(matches) == ["a.py"]


def test_glob_not_in_search_and_files(mixed_tree: Path):
    result = search(["hit"], paths=[str(mixed_tree)], glob_not=["*.py", "*.txt"])
    assert len(result) == 1
    assert "b.toml" in result[0]
    listed = files(paths=[str(mixed_tree)], glob_not=["*.txt"])
    assert sorted(Path(f).name for f in listed) == ["a.py", "b.toml"]


def test_max_bytes_limits_each_file(mixed_tree: Path):
    # c.txt's match starts after byte 50, the others are within the first 4 bytes.
    matches = search_structured(["hit"], paths=[str(mixed_tree)], max_bytes=20)
    assert _names(matches) == ["a.py", "b.toml"]
    assert len(search(["hit"], paths=[str(mixed_tree)], max_bytes=20)) == 2
    assert _names(search_structured(["hit"], paths=[str(mixed_tree)])) == ["a.py", "b.toml", "c.txt"]


@pytest.fixture
def many_files(tmp_path: Path) -> Path:
    for i in range(40):
        sub = tmp_path / f"d{i % 4}"
        sub.mkdir(exist_ok=True)
        (sub / f"f{i}.txt").write_bytes(b"x\nhit one\nhit two\n")
    return tmp_path


def test_search_structured_unsorted_matches_sorted(many_files: Path):
    """The (parallel) unsorted path finds the same matches as the sorted one."""
    from ripgrep_rs import PySortMode, PySortModeKind

    sort = PySortMode(kind=PySortModeKind.Path)
    unsorted = search_structured(["hit"], paths=[str(many_files)])
    ordered = search_structured(["hit"], paths=[str(many_files)], sort=sort)
    keys = lambda ms: sorted((m.path, m.line_number, m.line_text) for m in ms)  # noqa: E731
    assert len(ordered) == 80
    assert keys(unsorted) == keys(ordered)


def test_search_structured_max_total_unsorted(many_files: Path):
    matches = search_structured(["hit"], paths=[str(many_files)], max_total=5)
    assert len(matches) == 5
    # One match per file still respects max_count with max_total.
    matches = search_structured(["hit"], paths=[str(many_files)], max_total=7, max_count=1)
    assert len(matches) == 7
    assert len({m.path for m in matches}) == 7


def test_search_json_unsorted_matches_sorted(many_files: Path):
    from ripgrep_rs import PySortMode, PySortModeKind

    sort = PySortMode(kind=PySortModeKind.Path)
    unsorted = search(["hit"], paths=[str(many_files)], json=True)
    ordered = search(["hit"], paths=[str(many_files)], json=True, sort=sort)
    # "end" messages carry timings, compare everything else.
    lines = lambda out: sorted(  # noqa: E731
        line for chunk in out for line in chunk.splitlines() if '"type":"end"' not in line
    )
    assert len(ordered) == 40
    assert lines(unsorted) == lines(ordered)
