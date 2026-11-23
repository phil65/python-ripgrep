# ripgrep-rs

[![PyPI](https://img.shields.io/pypi/v/ripgrep-rs.svg)](https://pypi.org/project/ripgrep-rs/)
[![Python versions](https://img.shields.io/pypi/pyversions/ripgrep-rs.svg)](https://pypi.org/project/ripgrep-rs/)
[![CI](https://github.com/phil65/python-ripgrep/actions/workflows/ci.yml/badge.svg)](https://github.com/phil65/python-ripgrep/actions/workflows/ci.yml)

ripgrep's search and file walking as a native Python extension: regex search and `.gitignore`-aware file listing, without spawning an `rg` process.

## Description

ripgrep-rs is a Python package that wraps the functionality of ripgrep, a line-oriented search tool that recursively searches directories for a regex pattern. This package allows you to harness the power and speed of ripgrep directly from your Python code.

Unlike many other ripgrep bindings for Python, ripgrep-rs doesn't shell out to the ripgrep command-line tool. Instead, it reimplements core ripgrep logic in Rust and provides a direct interface to Python via [PyO3](https://pyo3.rs/).

## Features

- `search()`: ripgrep-style formatted output (optionally JSON Lines), with context lines, headings and line numbers
- `search_structured()`: typed `SearchMatch` objects with line numbers, byte offsets and submatches
- `files()` / `files_with_info()`: fast directory walks that honor `.gitignore`, hidden-file and glob rules; `files_with_info()` returns fsspec-style metadata
- Searches run multi-threaded with the GIL released; wheels for CPython 3.12-3.15 on Linux, macOS and Windows, plus free-threaded 3.14t/3.15t on Linux and macOS
- Ships type stubs (`py.typed`)

## Installation

```bash
pip install ripgrep-rs
```

The distribution is called `ripgrep-rs`; the import name is `ripgrep_rs`.

## Usage

### Text search

```python
from ripgrep_rs import search

# Returns one formatted string per file ("path:line" for each matching line).
results = search(
    patterns=["pattern"],
    paths=["path/to/search"],
    globs=["*.py"],
)
for result in results:
    print(result)
```

### Structured search

```python
from ripgrep_rs import search_structured

# Returns parsed SearchMatch objects instead of raw strings.
matches = search_structured(
    patterns=["def "],
    paths=["src/"],
    globs=["*.py"],
    max_total=10,
)
for m in matches:
    print(f"{m.path}:{m.line_number}: {m.line_text}")
    for sm in m.submatches:
        print(f"  match: [{sm.start}:{sm.end}] {sm.text!r}")
```

### File listing

```python
from ripgrep_rs import files, files_with_info

# List files respecting .gitignore
file_list = files(
    paths=["src/"],
    globs=["*.py"],
    hidden=False,
)

# List files with metadata (size, mtime, permissions, etc.)
file_info = files_with_info(
    paths=["src/"],
    globs=["*.py"],
    absolute=True,
)
for path, info in file_info.items():
    print(f"{path}: {info.size} bytes, modified {info.mtime}")
```

Paths are searched in parallel, so results come back in no particular order unless you pass `sort`:

```python
from ripgrep_rs import PySortMode, PySortModeKind, files

newest_first = files(
    paths=["src/"],
    sort=PySortMode(PySortModeKind.LastModified, reverse=True),
)
```

Invalid arguments (for example a malformed regex) raise `ValueError`.

## API Reference

### Functions

#### `search(patterns, ...)`

Search for regex patterns in files, returning raw formatted string results.

| Parameter | Type | Description |
|-----------|------|-------------|
| `patterns` | `list[str]` | Regex patterns to search for |
| `paths` | `list[str] \| None` | Paths to search in |
| `globs` | `list[str] \| None` | Glob patterns to filter files |
| `glob_not` | `list[str] \| None` | Glob patterns of files to exclude (like `rg -G`); applied after `globs`, so exclusions win |
| `heading` | `bool \| None` | Show file names above matching lines |
| `after_context` | `int \| None` | Lines to show after each match |
| `before_context` | `int \| None` | Lines to show before each match |
| `separator_field_context` | `str \| None` | Field separator for context lines (default `-`) |
| `separator_field_match` | `str \| None` | Field separator for matching lines (default `:`) |
| `separator_context` | `str \| None` | Separator between non-contiguous context blocks (default `--`) |
| `sort` | `PySortMode \| None` | Sort mode for results |
| `max_count` | `int \| None` | Maximum matches per file |
| `max_bytes` | `int \| None` | Search at most this many bytes of each file |
| `line_number` | `bool \| None` | Show line numbers |
| `multiline` | `bool \| None` | Enable multiline matching |
| `case_sensitive` | `bool \| None` | Force case-sensitive matching |
| `smart_case` | `bool \| None` | Case-insensitive unless pattern has uppercase |
| `no_ignore` | `bool \| None` | Don't respect .gitignore |
| `hidden` | `bool \| None` | Search hidden files |
| `json` | `bool \| None` | Return JSON Lines format |

#### `search_structured(patterns, ...)`

Search for regex patterns, returning parsed `SearchMatch` objects.

| Parameter | Type | Description |
|-----------|------|-------------|
| `patterns` | `list[str]` | Regex patterns to search for |
| `paths` | `list[str] \| None` | Paths to search in |
| `globs` | `list[str] \| None` | Glob patterns to filter files |
| `glob_not` | `list[str] \| None` | Glob patterns of files to exclude (like `rg -G`); applied after `globs`, so exclusions win |
| `sort` | `PySortMode \| None` | Sort mode for results |
| `max_count` | `int \| None` | Maximum matches per file |
| `max_bytes` | `int \| None` | Search at most this many bytes of each file |
| `max_total` | `int \| None` | Maximum total matches across all files |
| `multiline` | `bool \| None` | Enable multiline matching |
| `case_sensitive` | `bool \| None` | Force case-sensitive matching |
| `smart_case` | `bool \| None` | Case-insensitive unless pattern has uppercase |
| `no_ignore` | `bool \| None` | Don't respect .gitignore |
| `hidden` | `bool \| None` | Search hidden files |

#### `files(...)`

List files that would be searched, respecting ignore rules.

| Parameter | Type | Description |
|-----------|------|-------------|
| `paths` | `list[str] \| None` | Paths to search in |
| `globs` | `list[str] \| None` | Glob patterns to filter files |
| `glob_not` | `list[str] \| None` | Glob patterns of files to exclude (like `rg -G`); applied after `globs`, so exclusions win |
| `sort` | `PySortMode \| None` | Sort mode for results |
| `max_count` | `int \| None` | Maximum number of files |
| `no_ignore` | `bool \| None` | Don't respect .gitignore |
| `hidden` | `bool \| None` | Include hidden files |
| `include_dirs` | `bool \| None` | Include directories in results |
| `max_depth` | `int \| None` | Maximum directory depth |
| `absolute` | `bool \| None` | Return absolute paths |
| `relative_to` | `str \| None` | Strip this prefix from paths |

#### `files_with_info(...)`

List files with detailed metadata, returning a dict mapping paths to `FileInfo` objects.

Same parameters as `files()`, except `max_count`.

### Classes

#### `SearchMatch`

| Attribute | Type | Description |
|-----------|------|-------------|
| `path` | `str` | File path |
| `line_number` | `int` | 1-based line number |
| `absolute_offset` | `int` | Byte offset from file start |
| `line_text` | `str` | Text of the matching line |
| `submatches` | `list[SearchSubmatch]` | Individual submatches |

#### `SearchSubmatch`

| Attribute | Type | Description |
|-----------|------|-------------|
| `text` | `str` | The matched text |
| `start` | `int` | Byte offset of match start within the line |
| `end` | `int` | Byte offset of match end within the line |

#### `FileInfo`

| Attribute | Type | Description |
|-----------|------|-------------|
| `name` | `str` | File path |
| `size` | `int` | File size in bytes |
| `type` | `str` | `"file"` or `"directory"` |
| `created` | `float` | Creation time (Unix timestamp) |
| `islink` | `bool` | Whether the file is a symlink |
| `mode` | `int` | File permissions |
| `uid` | `int` | User ID (Unix only) |
| `gid` | `int` | Group ID (Unix only) |
| `mtime` | `float` | Modification time (Unix timestamp) |
| `ino` | `int` | Inode number (Unix only) |
| `nlink` | `int` | Number of hard links |

#### `PySortMode`

```python
PySortMode(kind=PySortModeKind.Path, reverse=False)
```

#### `PySortModeKind`

PyO3 enum (not an `enum.Enum`) with members `Path`, `LastModified`, `LastAccessed`, `Created`.

## Development

This project uses [maturin](https://github.com/PyO3/maturin) for building the Python package from Rust code.

```bash
git clone https://github.com/phil65/python-ripgrep
cd python-ripgrep
python -m venv .venv && source .venv/bin/activate
pip install maturin pytest
maturin develop
pytest tests/
```

See [AGENTS.md](AGENTS.md) for the code layout.

## License

MIT License - see [LICENSE](LICENSE) file for details.

## Acknowledgements

This project is based on [ripgrep](https://github.com/BurntSushi/ripgrep) by Andrew Gallant.
