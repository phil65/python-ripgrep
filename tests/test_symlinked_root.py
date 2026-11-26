"""`absolute=True` under a symlinked root keeps paths lexical (inside the root asked for)."""

import os
from pathlib import Path

import pytest

from ripgrep_rs import files, files_with_info, search_structured


@pytest.fixture
def linked_root(tmp_path: Path) -> Path:
    real = tmp_path / "d"
    (real / "sub").mkdir(parents=True)
    (real / "a.txt").write_bytes(b"hit\n")
    (real / "sub" / "b.txt").write_bytes(b"hit\n")
    link = tmp_path / "dl"
    try:
        os.symlink(real, link, target_is_directory=True)
    except OSError:
        pytest.skip("symlinks not available")
    return link


def test_files_absolute_relative_to_symlinked_root(linked_root: Path):
    root = str(linked_root)
    result = files(paths=[root], absolute=True, relative_to=root)
    assert sorted(result) == ["a.txt", os.path.join("sub", "b.txt")]


def test_files_absolute_stays_under_symlinked_root(linked_root: Path):
    root = str(linked_root)
    result = files(paths=[root], absolute=True)
    assert sorted(result) == [
        os.path.join(root, "a.txt"),
        os.path.join(root, "sub", "b.txt"),
    ]


def test_files_with_info_absolute_symlinked_root(linked_root: Path):
    root = str(linked_root)
    result = files_with_info(paths=[root], absolute=True, relative_to=root)
    assert sorted(result) == ["a.txt", os.path.join("sub", "b.txt")]


def test_files_absolute_relative_root_joins_cwd(linked_root: Path, monkeypatch):
    monkeypatch.chdir(linked_root.parent)
    result = files(paths=["dl"], absolute=True, sort=None)
    cwd = os.getcwd()
    assert sorted(result) == [
        os.path.join(cwd, "dl", "a.txt"),
        os.path.join(cwd, "dl", "sub", "b.txt"),
    ]


def test_files_absolute_normalizes_dot_dot(linked_root: Path):
    root = os.path.join(str(linked_root), "sub", "..")
    result = files(paths=[root], absolute=True)
    assert sorted(result) == [
        os.path.join(str(linked_root), "a.txt"),
        os.path.join(str(linked_root), "sub", "b.txt"),
    ]


def test_search_structured_under_symlinked_root(linked_root: Path):
    matches = search_structured(["hit"], paths=[str(linked_root)])
    assert sorted(Path(m.path).name for m in matches) == ["a.txt", "b.txt"]
