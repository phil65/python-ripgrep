"""Tests for symlink handling (`FileInfo.islink` and `follow` option)."""

from pathlib import Path

import pytest

from ripgrep_rs import (
    PySortMode,
    PySortModeKind,
    files,
    files_with_info,
    search,
    search_structured,
)


@pytest.fixture
def symlink_tree(tmp_path: Path) -> Path:
    """Create a directory tree with a real file, a file symlink, and a dir symlink."""
    root = tmp_path / "root"
    root.mkdir()
    external = tmp_path / "external"
    external.mkdir()

    real_file = root / "real.txt"
    real_file.write_bytes(b"real needle\n")

    nested_file = external / "nested.txt"
    nested_file.write_bytes(b"nested needle\n")

    try:
        (root / "link.txt").symlink_to(real_file)
        (root / "link_dir").symlink_to(external, target_is_directory=True)
    except OSError as exc:
        pytest.skip(f"symlinks not supported in this environment: {exc}")

    return root


def _rel_names(paths: list[str], root: Path) -> set[str]:
    return {Path(p).relative_to(root).as_posix() for p in paths}


def test_files_follow_false_does_not_descend_into_symlinked_dir(symlink_tree: Path):
    default_result = files(paths=[str(symlink_tree)])
    explicit_false = files(paths=[str(symlink_tree)], follow=False)

    assert _rel_names(default_result, symlink_tree) == {"real.txt"}
    assert _rel_names(explicit_false, symlink_tree) == {"real.txt"}


def test_files_follow_true_descends_into_symlinked_dir(symlink_tree: Path):
    result = files(paths=[str(symlink_tree)], follow=True)
    assert _rel_names(result, symlink_tree) == {
        "real.txt",
        "link.txt",
        "link_dir/nested.txt",
    }


@pytest.mark.parametrize(
    "sort",
    [None, PySortMode(PySortModeKind.Path)],
    ids=["parallel", "sorted"],
)
def test_files_with_info_islink_follow_true(symlink_tree: Path, sort: PySortMode | None):
    info = files_with_info(
        paths=[str(symlink_tree)],
        include_dirs=True,
        follow=True,
        sort=sort,
    )
    by_rel = {Path(p).relative_to(symlink_tree).as_posix(): fi for p, fi in info.items()}

    assert set(by_rel) == {"real.txt", "link.txt", "link_dir", "link_dir/nested.txt"}

    assert by_rel["real.txt"].islink is False
    assert by_rel["link.txt"].islink is True
    assert by_rel["link_dir"].islink is True
    assert by_rel["link_dir/nested.txt"].islink is False

    # Other fields still describe the target.
    assert by_rel["link.txt"].type == "file"
    assert by_rel["link.txt"].size == len(b"real needle\n")
    assert by_rel["link_dir"].type == "directory"


def test_files_with_info_follow_false(symlink_tree: Path):
    info = files_with_info(
        paths=[str(symlink_tree)],
        include_dirs=True,
        follow=False,
    )
    by_rel = {Path(p).relative_to(symlink_tree).as_posix(): fi for p, fi in info.items()}

    assert "real.txt" in by_rel
    assert "link_dir/nested.txt" not in by_rel
    assert by_rel["real.txt"].islink is False
    if "link_dir" in by_rel:
        assert by_rel["link_dir"].islink is True
        assert by_rel["link_dir"].type == "directory"


def test_search_follow(symlink_tree: Path):
    without_follow = search(["needle"], paths=[str(symlink_tree)])
    without_follow_explicit = search(["needle"], paths=[str(symlink_tree)], follow=False)
    with_follow = search(["needle"], paths=[str(symlink_tree)], follow=True)

    assert len(without_follow) == 1
    assert "real.txt" in without_follow[0]
    assert len(without_follow_explicit) == 1
    assert len(with_follow) == 3
    joined = "".join(with_follow)
    assert "real.txt" in joined
    assert "link.txt" in joined
    assert "nested.txt" in joined


def test_search_structured_follow(symlink_tree: Path):
    without_follow = search_structured(["needle"], paths=[str(symlink_tree)])
    without_follow_explicit = search_structured(
        ["needle"], paths=[str(symlink_tree)], follow=False
    )
    with_follow = search_structured(["needle"], paths=[str(symlink_tree)], follow=True)

    assert _rel_names([m.path for m in without_follow], symlink_tree) == {"real.txt"}
    assert _rel_names([m.path for m in without_follow_explicit], symlink_tree) == {"real.txt"}
    assert _rel_names([m.path for m in with_follow], symlink_tree) == {
        "real.txt",
        "link.txt",
        "link_dir/nested.txt",
    }
