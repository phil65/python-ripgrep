"""Tests for search() and search_structured()."""

from pathlib import Path

import pytest

from ripgrep_rs import search, search_structured


@pytest.fixture
def tree(tmp_path: Path) -> Path:
    (tmp_path / "a.txt").write_text("foo 1\nfoo 2\nfoo 3\nfoo 4\n")
    (tmp_path / "b.txt").write_text("bar\nfoo 5\n")
    return tmp_path


def test_search_max_count(tree: Path):
    result = search(["foo"], paths=[str(tree / "a.txt")], max_count=1)
    assert result == ["foo 1\n"]


def test_search_structured_max_count(tree: Path):
    matches = search_structured(["foo"], paths=[str(tree)], max_count=1)
    per_file = {Path(m.path).name: m.line_number for m in matches}
    assert per_file == {"a.txt": 1, "b.txt": 2}
    assert len(matches) == 2
