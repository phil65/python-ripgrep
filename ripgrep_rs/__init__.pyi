import enum

class PySortMode:
    kind: PySortModeKind
    reverse: bool = False

    def __init__(self, kind: PySortModeKind, reverse: bool = False): ...

class PySortModeKind(enum.Enum):
    Path = enum.auto()
    LastModified = enum.auto()
    LastAccessed = enum.auto()
    Created = enum.auto()

class SearchSubmatch:
    """A single submatch within a search match line."""

    text: str
    """The matched text."""
    start: int
    """Byte offset of match start within the line."""
    end: int
    """Byte offset of match end within the line."""

class SearchMatch:
    """A single search match (one matching line)."""

    path: str
    """File path (relative or absolute depending on search args)."""
    line_number: int
    """1-based line number."""
    absolute_offset: int
    """Absolute byte offset from the start of the file."""
    line_text: str
    """The text of the matching line (trimmed trailing newline)."""
    submatches: list[SearchSubmatch]
    """Individual submatches within this line."""

def search(
    patterns: list[str],
    paths: list[str] | None = None,
    globs: list[str] | None = None,
    heading: bool | None = None,
    after_context: int | None = None,
    before_context: int | None = None,
    separator_field_context: str | None = None,
    separator_field_match: str | None = None,
    separator_context: str | None = None,
    sort: PySortMode | None = None,
    max_count: int | None = None,
    line_number: bool | None = None,
    multiline: bool | None = None,
    case_sensitive: bool | None = None,
    smart_case: bool | None = None,
    no_ignore: bool | None = None,
    hidden: bool | None = None,
    json: bool | None = None,
) -> list[str]: ...
def search_structured(
    patterns: list[str],
    paths: list[str] | None = None,
    globs: list[str] | None = None,
    sort: PySortMode | None = None,
    max_count: int | None = None,
    max_total: int | None = None,
    multiline: bool | None = None,
    case_sensitive: bool | None = None,
    smart_case: bool | None = None,
    no_ignore: bool | None = None,
    hidden: bool | None = None,
) -> list[SearchMatch]:
    """Search for patterns and return structured match objects.

    Unlike `search()`, this returns parsed `SearchMatch` objects instead of
    raw formatted strings. Each match contains the file path, line number,
    line text, and individual submatches with byte offsets.

    Args:
        patterns: Regex patterns to search for.
        paths: Paths to search in.
        globs: Glob patterns to filter files.
        sort: Sort mode for results.
        max_count: Maximum matches per file.
        max_total: Maximum total matches across all files.
        multiline: Enable multiline matching.
        case_sensitive: Force case-sensitive matching.
        smart_case: Enable smart case (case-insensitive unless pattern has uppercase).
        no_ignore: Don't respect .gitignore and other ignore files.
        hidden: Search hidden files and directories.
    """
    ...

def files(
    paths: list[str] | None = None,
    globs: list[str] | None = None,
    sort: PySortMode | None = None,
    max_count: int | None = None,
    no_ignore: bool | None = None,
    hidden: bool | None = None,
    include_dirs: bool | None = None,
    max_depth: int | None = None,
    absolute: bool | None = None,
    relative_to: str | None = None,
) -> list[str]: ...

class FileInfo:
    """File information returned by files_with_info."""

    name: str
    """Full path to the file."""
    size: int
    """File size in bytes."""
    type: str
    """File type: 'file' or 'directory'."""
    created: float
    """Creation time as Unix timestamp."""
    islink: bool
    """Whether the file is a symlink."""
    mode: int
    """File mode/permissions."""
    uid: int
    """User ID (Unix only, 0 on Windows)."""
    gid: int
    """Group ID (Unix only, 0 on Windows)."""
    mtime: float
    """Modification time as Unix timestamp."""
    ino: int
    """Inode number (Unix only, 0 on Windows)."""
    nlink: int
    """Number of hard links."""

def files_with_info(
    paths: list[str] | None = None,
    globs: list[str] | None = None,
    sort: PySortMode | None = None,
    no_ignore: bool | None = None,
    hidden: bool | None = None,
    include_dirs: bool | None = None,
    max_depth: int | None = None,
    absolute: bool | None = None,
    relative_to: str | None = None,
) -> dict[str, FileInfo]:
    """List files with detailed metadata.

    Returns a dictionary mapping file paths to FileInfo objects,
    similar to fsspec's detail=True mode.
    """
    ...
