# AGENTS.md

## Overview

`ripgrep-rs` (import name `ripgrep_rs`, repo `phil65/python-ripgrep`) is a PyO3/maturin
extension that embeds ripgrep's search and directory-walking logic (the `grep`, `ignore`,
`grep-*` crates plus an adapted copy of ripgrep's argument layer) and exposes it to Python
without spawning `rg`. Used by `upathtools` (fsspec local filesystem `find`/`grep`) and agentpool.
Not copier-managed.

## Architecture

Rust <-> Python boundary:

- `src/lib.rs`: the `#[pymodule] ripgrep_rs` (declared `gil_used = false`, i.e. free-threading
  safe). Registers the classes and the four functions. The compiled module is
  `ripgrep_rs.ripgrep_rs`.
- `src/ripgrep_core.rs`: all PyO3 bindings and glue.
  - `#[pyclass]` types: `PyArgs` (internal argument bag), `PySortMode`, `PySortModeKind`
    (simple PyO3 enum), `FileInfo`, `SearchMatch`, `SearchSubmatch`.
  - `#[pyfunction]`s: `py_search` -> `search`, `py_search_structured` -> `search_structured`,
    `py_files` -> `files`, `py_files_with_info` -> `files_with_info`. Each builds a `PyArgs`,
    then releases the GIL with `py.detach(...)`.
  - `pyargs_to_hiargs()`: maps `PyArgs` onto `LowArgs` (ripgrep's parsed-flag struct) and calls
    `HiArgs::from_low_args`. New options go here.
  - `py_search_impl*` / `py_files_impl*` / `py_files_with_info_impl*`: sequential and parallel
    (`ignore` walker) variants. Output is collected into in-memory buffers, not stdout.
    `search_structured` collects matches with its own `Sink` (`structured.rs`), no JSON round-trip.
  - Path handling: `normalize_path`, `normalize_separators`, `normalize_and_strip`,
    `build_relative_prefix` (absolute paths, `relative_to`, Windows `\\?\` and separators).
  - Errors from argument building or searching are raised as `ValueError`.
- `src/ripgrep_core/structured.rs`: `StructuredSink`, builds `SearchMatch` values directly; its
  submatch logic mirrors grep-printer's JSON sink, so results match `json=True`.
- `src/ripgrep_core/{lowargs,hiargs,haystack,search,messages}.rs`: adapted from ripgrep's
  `crates/core`. `lowargs.rs` = flag values and defaults, `hiargs.rs` = derived config (matcher,
  printer, walker builders), `search.rs` = search worker, `haystack.rs` = which entries get
  searched, `messages.rs` = error/message macros.
- `src/ripgrep_core/main.rs`: upstream ripgrep entry point kept for reference. **Not compiled**
  (no `mod main;`), so edits there have no effect.

Python side:

- `ripgrep_rs/__init__.py`: re-exports from `.ripgrep_rs`.
- `ripgrep_rs/__init__.pyi`: hand-written stubs, the API users and type checkers see.
  `ripgrep_rs/py.typed` ships with the wheel.

## Where to look

| Task | File |
| --- | --- |
| Add/change a keyword argument | `#[pyo3(signature)]` + fn params in `src/ripgrep_core.rs`, `PyArgs`, `pyargs_to_hiargs`, `ripgrep_rs/__init__.pyi`, README API table |
| Change result objects | `FileInfo` / `SearchMatch` / `SearchSubmatch` in `src/ripgrep_core.rs` + stub |
| Register a new function/class | `src/lib.rs`, `ripgrep_rs/__init__.py` (`__all__`), stub `__all__` |
| Search/printer behavior, defaults | `src/ripgrep_core/lowargs.rs`, `src/ripgrep_core/hiargs.rs` |
| Ignore/hidden/glob filtering | `hiargs.rs` (walk builder), `haystack.rs` |
| Path normalization, `absolute`, `relative_to` | `normalize_*` helpers in `src/ripgrep_core.rs` |
| Tests | `tests/test_search.py`, `tests/test_absolute_paths.py`, smoke script `test.py` |
| Benchmarks | `benchmarks/bench.py` |
| CI / wheels | `.github/workflows/ci.yml`, `.github/workflows/release.yml` |

## Commands

```bash
python -m venv .venv && source .venv/bin/activate
pip install -U pip maturin pytest
maturin develop                 # build + install the extension (debug); add --release for speed
python test.py                  # smoke script (CI runs it)
pytest tests/ -v                # CI test suite
cargo fmt --all -- --check      # CI lint job
cargo clippy --all-targets --all-features -- -D warnings
cargo test                      # compiles; there are no Rust unit tests
python -P benchmarks/bench.py   # benchmarks; -P so the source dir doesn't shadow the build
python -m mypy.stubtest ripgrep_rs --ignore-missing-stub   # stub vs built module (needs mypy)
```

`duties.py` wraps some of these (`duty develop`, `duty lint`, `duty lint_check`, `duty update`).

## Conventions

- Version lives only in `Cargo.toml` (`pyproject.toml` has `dynamic = ["version"]`).
- All Python-facing parameters are optional keywords defaulting to `None`; "not set" means the
  ripgrep default from `lowargs.rs`. Keep the stub signatures identical to `#[pyo3(signature)]`.
- Stub conventions: classes are `@final`, pyclass fields (`get_all`) are read-only
  `@property`s, `PySortMode` uses `__new__`, `PySortModeKind` is a PyO3 enum (ClassVar
  members, not `enum.Enum`). Re-run stubtest after changing the API.
- Rust edition 2024; clippy runs with `-D warnings`, and allowed lints are listed in
  `Cargo.toml` `[lints.clippy]`.
- Tests write fixtures as bytes (avoids CRLF translation on Windows) and must pass on
  Linux, macOS and Windows for Python 3.12-3.15.

## Pitfalls

- Output must not depend on the environment: `search()` defaults (heading, line numbers,
  colors) don't depend on whether stdout is a tty, and stdin is never read when `paths` is
  omitted (the current directory is searched instead). Tests cover both.
- `max_count` is per file for search (enforced in the search loop, since grep-searcher no
  longer does it in the printer); `max_total` (structured only) caps matches overall.
  `files_with_info` has no `max_count`.
- Unsorted results come from the parallel walker and their order is nondeterministic. Tests
  should sort or pass `sort=`.
- `files()` filters out the root directories themselves and normalizes separators. Keep
  upathtools' expectations (`tests/test_absolute_paths.py`) passing.
- Release builds free-threaded wheels (3.14t/3.15t; 3.13t is not built because current
  maturin doesn't support it). Don't introduce global mutable state without synchronization.
- Releases: publishing a GitHub release triggers `release.yml` -> PyPI (trusted publishing).
  `Cargo.lock` and `uv.lock` are tracked even though `.gitignore` lists `Cargo.lock`.
