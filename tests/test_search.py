"""Tests for search() and search_structured()."""

import os
import subprocess
import sys
from pathlib import Path

import pytest

from ripgrep_rs import search, search_structured


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
