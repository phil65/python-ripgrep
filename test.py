from ripgrep_rs import (
    PySortMode,
    PySortModeKind,
    files,
    search,
    search_structured,
)


def test_search_with_kwargs() -> None:
    print("Testing search with keyword arguments:")
    results = search(patterns=["def"], paths=["src"], globs=["*.rs"], line_number=True)
    print(f"Found {len(results)} matches")
    for result in results:
        print(result)
    print()


def test_files_with_kwargs() -> None:
    print("Testing files with keyword arguments:")
    results = files(
        paths=["src"],
        globs=["*.rs"],
        sort=PySortMode(PySortModeKind.Path),
    )
    print(f"Found {len(results)} files")
    for result in results:
        print(result)
    print()


def test_search_structured() -> None:
    print("Testing search_structured:")
    results = search_structured(["def"], paths=["src"], globs=["*.rs"], max_total=5)
    print(f"Found {len(results)} matches")
    for m in results:
        print(f"  {m.path}:{m.line_number}: {m.line_text.strip()}")
        for sm in m.submatches:
            print(f"    submatch: [{sm.start}:{sm.end}] {sm.text!r}")
    print()


if __name__ == "__main__":
    test_search_with_kwargs()
    test_files_with_kwargs()
    test_search_structured()
