use hiargs::HiArgs;
use ignore::WalkState;
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::{OsStr, OsString},
    path::{Component, Path, PathBuf},
    sync::mpsc,
    thread,
    time::UNIX_EPOCH,
};

mod haystack;
mod hiargs;
mod lowargs;
mod search;
mod structured;
#[macro_use]
mod messages;

use structured::StructuredSink;

#[pyclass]
pub struct PyArgs {
    pub patterns: Vec<String>,
    pub paths: Option<Vec<String>>,
    pub globs: Option<Vec<String>>,
    pub glob_not: Option<Vec<String>>,
    pub heading: Option<bool>,
    pub after_context: Option<u64>,
    pub before_context: Option<u64>,
    pub separator_field_context: Option<String>,
    pub separator_field_match: Option<String>,
    pub separator_context: Option<String>,
    pub sort: Option<PySortMode>,
    pub max_count: Option<u64>,
    pub max_bytes: Option<u64>,
    pub line_number: Option<bool>,
    pub multiline: Option<bool>,
    pub case_sensitive: Option<bool>,
    pub smart_case: Option<bool>,
    pub no_ignore: Option<bool>,
    pub hidden: Option<bool>,
    pub json: Option<bool>,
    pub include_dirs: Option<bool>,
    pub max_depth: Option<usize>,
    pub absolute: Option<bool>,
    pub relative_to: Option<String>,
    pub follow: bool,
}

#[pymethods]
impl PyArgs {
    #[new]
    #[pyo3(signature = (
        patterns,
        paths=None,
        globs=None,
        glob_not=None,
        heading=None,
        after_context=None,
        before_context=None,
        separator_field_context=None,
        separator_field_match=None,
        separator_context=None,
        sort=None,
        max_count=None,
        max_bytes=None,
        line_number=None,
        multiline=None,
        case_sensitive=None,
        smart_case=None,
        no_ignore=None,
        hidden=None,
        json=None,
        include_dirs=None,
        max_depth=None,
        absolute=None,
        relative_to=None,
        follow=false,
    ))]
    fn new(
        patterns: Vec<String>,
        paths: Option<Vec<String>>,
        globs: Option<Vec<String>>,
        glob_not: Option<Vec<String>>,
        heading: Option<bool>,
        after_context: Option<u64>,
        before_context: Option<u64>,
        separator_field_context: Option<String>,
        separator_field_match: Option<String>,
        separator_context: Option<String>,
        sort: Option<PySortMode>,
        max_count: Option<u64>,
        max_bytes: Option<u64>,
        line_number: Option<bool>,
        multiline: Option<bool>,
        case_sensitive: Option<bool>,
        smart_case: Option<bool>,
        no_ignore: Option<bool>,
        hidden: Option<bool>,
        json: Option<bool>,
        include_dirs: Option<bool>,
        max_depth: Option<usize>,
        absolute: Option<bool>,
        relative_to: Option<String>,
        follow: bool,
    ) -> Self {
        PyArgs {
            patterns,
            paths,
            globs,
            glob_not,
            heading,
            after_context,
            before_context,
            separator_field_context,
            separator_field_match,
            separator_context,
            sort,
            max_count,
            max_bytes,
            line_number,
            multiline,
            case_sensitive,
            smart_case,
            no_ignore,
            hidden,
            json,
            include_dirs,
            max_depth,
            absolute,
            relative_to,
            follow,
        }
    }
}

/// File information returned by files_with_info
#[pyclass(get_all, skip_from_py_object)]
#[derive(Clone)]
pub struct FileInfo {
    /// Full path to the file
    pub name: String,
    /// File size in bytes
    pub size: u64,
    /// File type: "file" or "directory"
    #[pyo3(name = "type")]
    pub file_type: String,
    /// Creation time as Unix timestamp (may equal mtime on some platforms)
    pub created: f64,
    /// Whether the file is a symlink
    pub islink: bool,
    /// File mode/permissions
    pub mode: u32,
    /// User ID (Unix only, 0 on Windows)
    #[cfg(unix)]
    pub uid: u32,
    #[cfg(not(unix))]
    pub uid: u32,
    /// Group ID (Unix only, 0 on Windows)
    #[cfg(unix)]
    pub gid: u32,
    #[cfg(not(unix))]
    pub gid: u32,
    /// Modification time as Unix timestamp
    pub mtime: f64,
    /// Inode number (Unix only, 0 on Windows)
    #[cfg(unix)]
    pub ino: u64,
    #[cfg(not(unix))]
    pub ino: u64,
    /// Number of hard links
    pub nlink: u64,
}

/// A single submatch within a search match line
#[pyclass(get_all, skip_from_py_object)]
#[derive(Clone)]
pub struct SearchSubmatch {
    /// The matched text
    pub text: String,
    /// Byte offset of match start within the line
    pub start: usize,
    /// Byte offset of match end within the line
    pub end: usize,
}

/// A single search match (one matching line)
#[pyclass(get_all, skip_from_py_object)]
#[derive(Clone)]
pub struct SearchMatch {
    /// File path (relative or absolute depending on search args)
    pub path: String,
    /// 1-based line number
    pub line_number: u64,
    /// Absolute byte offset from the start of the file
    pub absolute_offset: u64,
    /// The text of the matching line (trimmed trailing newline)
    pub line_text: String,
    /// Individual submatches within this line
    pub submatches: Vec<SearchSubmatch>,
}

#[pyclass(eq, from_py_object)]
#[derive(PartialEq, Clone)]
#[pyo3(get_all)]
pub struct PySortMode {
    pub kind: PySortModeKind,
    pub reverse: bool,
}

#[pymethods]
impl PySortMode {
    #[new]
    #[pyo3(signature = (kind, reverse=false))]
    fn new(kind: PySortModeKind, reverse: bool) -> Self {
        PySortMode { kind, reverse }
    }
}

#[pyclass(eq, from_py_object)]
#[derive(PartialEq, Clone)]
#[pyo3(get_all)]
pub enum PySortModeKind {
    Path,
    LastModified,
    LastAccessed,
    Created,
}

fn build_patterns(patterns: Vec<String>) -> Vec<lowargs::PatternSource> {
    patterns
        .into_iter()
        .map(|pattern| lowargs::PatternSource::Regexp(pattern))
        .collect()
}

fn build_paths(paths: Vec<String>) -> Vec<OsString> {
    paths.into_iter().map(|path| OsString::from(path)).collect()
}

fn build_sort_mode_kind(kind: PySortModeKind) -> lowargs::SortModeKind {
    match kind {
        PySortModeKind::Path => lowargs::SortModeKind::Path,
        PySortModeKind::LastModified => lowargs::SortModeKind::LastModified,
        PySortModeKind::LastAccessed => lowargs::SortModeKind::LastAccessed,
        PySortModeKind::Created => lowargs::SortModeKind::Created,
    }
}

fn build_sort_mode(sort: Option<PySortMode>) -> Option<lowargs::SortMode> {
    if let Some(sort_mode) = sort {
        Some(lowargs::SortMode {
            kind: build_sort_mode_kind(sort_mode.kind),
            reverse: sort_mode.reverse,
        })
    } else {
        None
    }
}

fn build_context_mode(
    after_context: Option<u64>,
    before_context: Option<u64>,
) -> lowargs::ContextMode {
    let mut context_mode = lowargs::ContextMode::default();

    if let Some(after) = after_context {
        context_mode.set_after(after as usize);
    }

    if let Some(before) = before_context {
        context_mode.set_before(before as usize);
    }

    context_mode
}

fn pyargs_to_hiargs(py_args: &PyArgs, mode: lowargs::Mode) -> anyhow::Result<HiArgs> {
    let mut low_args = lowargs::LowArgs::default();

    low_args.patterns = build_patterns(py_args.patterns.clone());

    low_args.mode = mode;

    low_args.sort = build_sort_mode(py_args.sort.clone());

    low_args.heading = py_args.heading;

    low_args.max_count = py_args.max_count;

    low_args.line_number = py_args.line_number;

    low_args.context = build_context_mode(py_args.after_context, py_args.before_context);

    if let Some(globs) = &py_args.globs {
        low_args.globs = globs.clone();
    }

    // Like ripgrep's -G/--glob-not: "x" excludes x, "!x" un-negates. Appended after
    // `globs` so exclusions win (the last matching glob takes precedence).
    if let Some(glob_not) = &py_args.glob_not {
        low_args
            .globs
            .extend(glob_not.iter().map(|glob| match glob.strip_prefix('!') {
                Some(unnegated) => unnegated.to_string(),
                None => format!("!{glob}"),
            }));
    }

    low_args.max_bytes = py_args.max_bytes;

    if let Some(paths) = &py_args.paths {
        low_args.positional = build_paths(paths.clone());
    }

    if let Some(separator_field_context) = &py_args.separator_field_context {
        let sep = OsStr::new(separator_field_context);
        low_args.field_context_separator = lowargs::FieldContextSeparator::new(&sep).unwrap();
    }

    if let Some(separator_field_match) = &py_args.separator_field_match {
        let sep = OsStr::new(separator_field_match);
        low_args.field_match_separator = lowargs::FieldMatchSeparator::new(&sep).unwrap();
    }

    if let Some(separator_context) = &py_args.separator_context {
        let sep = OsStr::new(separator_context);
        low_args.context_separator = lowargs::ContextSeparator::new(&sep).unwrap();
    }

    if let Some(multiline) = py_args.multiline {
        low_args.multiline = multiline;
    }

    // Case sensitivity handling
    if let Some(true) = py_args.smart_case {
        low_args.case = lowargs::CaseMode::Smart;
    } else if let Some(false) = py_args.case_sensitive {
        low_args.case = lowargs::CaseMode::Insensitive;
    } else if let Some(true) = py_args.case_sensitive {
        low_args.case = lowargs::CaseMode::Sensitive;
    }

    // Ignore file handling
    if let Some(true) = py_args.no_ignore {
        low_args.no_ignore_dot = true;
        low_args.no_ignore_vcs = true;
        low_args.no_ignore_global = true;
        low_args.no_ignore_parent = true;
        low_args.no_ignore_files = true;
    }

    // Hidden files
    if let Some(hidden) = py_args.hidden {
        low_args.hidden = hidden;
    }

    // Include directories in results
    if let Some(include_dirs) = py_args.include_dirs {
        low_args.include_dirs = include_dirs;
    }

    // Max directory depth
    if let Some(max_depth) = py_args.max_depth {
        low_args.max_depth = Some(max_depth);
    }

    // Follow symbolic links
    low_args.follow = py_args.follow;

    HiArgs::from_low_args(low_args)
}

#[pyfunction]
#[pyo3(name = "search")]
#[pyo3(signature = (
    patterns,
    paths=None,
    globs=None,
    glob_not=None,
    heading=None,
    after_context=None,
    before_context=None,
    separator_field_context=None,
    separator_field_match=None,
    separator_context=None,
    sort=None,
    max_count=None,
    max_bytes=None,
    line_number=None,
    multiline=None,
    case_sensitive=None,
    smart_case=None,
    no_ignore=None,
    hidden=None,
    json=None,
    follow=false,
))]
pub fn py_search(
    py: Python<'_>,
    patterns: Vec<String>,
    paths: Option<Vec<String>>,
    globs: Option<Vec<String>>,
    glob_not: Option<Vec<String>>,
    heading: Option<bool>,
    after_context: Option<u64>,
    before_context: Option<u64>,
    separator_field_context: Option<String>,
    separator_field_match: Option<String>,
    separator_context: Option<String>,
    sort: Option<PySortMode>,
    max_count: Option<u64>,
    max_bytes: Option<u64>,
    line_number: Option<bool>,
    multiline: Option<bool>,
    case_sensitive: Option<bool>,
    smart_case: Option<bool>,
    no_ignore: Option<bool>,
    hidden: Option<bool>,
    json: Option<bool>,
    follow: bool,
) -> PyResult<Vec<String>> {
    py.detach(|| {
        let py_args = PyArgs {
            patterns,
            paths,
            globs,
            glob_not,
            heading,
            after_context,
            before_context,
            separator_field_context,
            separator_field_match,
            separator_context,
            sort,
            max_count,
            max_bytes,
            line_number,
            multiline,
            case_sensitive,
            smart_case,
            no_ignore,
            hidden,
            json,
            include_dirs: None, // search doesn't use this
            max_depth: None,    // search doesn't use this
            absolute: None,     // search doesn't use this
            relative_to: None,  // search doesn't use this
            follow,
        };

        let mode = if py_args.json == Some(true) {
            lowargs::Mode::Search(lowargs::SearchMode::JSON)
        } else {
            lowargs::Mode::default()
        };

        let args_result = pyargs_to_hiargs(&py_args, mode);

        if let Err(err) = args_result {
            return Err(PyValueError::new_err(err.to_string()));
        }

        let args = args_result.unwrap();

        let search_result = py_search_impl(&args);

        if let Err(err) = search_result {
            return Err(PyValueError::new_err(err.to_string()));
        }

        Ok(search_result.unwrap())
    })
}

fn py_search_impl(args: &HiArgs) -> anyhow::Result<Vec<String>> {
    // Check if JSON mode is requested
    if let lowargs::Mode::Search(lowargs::SearchMode::JSON) = args.mode() {
        return py_search_impl_json(args);
    }

    // Use parallel implementation when threads > 1 and no sorting requested
    if args.threads() > 1 && args.sort_mode().is_none() {
        return py_search_impl_parallel(args);
    }

    // Single-threaded implementation (also used when sorting is requested)
    let haystack_builder = args.haystack_builder();
    let unsorted = args
        .walk_builder()?
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));
    let haystacks = args.sort(unsorted);

    let args_matcher = args.matcher()?;
    let args_searcher = args.searcher()?;
    let args_printer = args.printer_no_color(vec![]);

    let mut results = Vec::new();

    let mut searcher = args.search_worker(args_matcher, args_searcher, args_printer)?;

    for haystack in haystacks {
        match searcher.search(&haystack) {
            Ok(_) => {}
            // A broken pipe means graceful termination.
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => break,
            // Keep what was printed before the error (ripgrep 16, #3382).
            Err(err) => {
                err_message!("{}: {}", haystack.path().display(), err);
            }
        }

        let buf = searcher.printer().get_mut().get_mut();
        // Only include results for valid UTF-8 output (skips binary or
        // differently encoded files).
        if !buf.is_empty()
            && let Ok(results_str) = String::from_utf8(buf.clone())
        {
            results.push(results_str);
        }
        buf.clear();
    }

    Ok(results)
}

/// Parallel search implementation
fn py_search_impl_parallel(args: &HiArgs) -> anyhow::Result<Vec<String>> {
    use std::sync::Mutex;

    let haystack_builder = args.haystack_builder();
    let results = Mutex::new(Vec::new());

    let searcher = args.search_worker(
        args.matcher()?,
        args.searcher()?,
        args.printer_no_color(vec![]),
    )?;

    args.walk_builder()?.build_parallel().run(|| {
        let haystack_builder = &haystack_builder;
        let results = &results;
        let mut searcher = searcher.clone();

        Box::new(move |result| {
            let haystack = match haystack_builder.build_from_result(result) {
                Some(haystack) => haystack,
                None => return WalkState::Continue,
            };

            // Clear the printer buffer for this file
            searcher.printer().get_mut().get_mut().clear();

            match searcher.search(&haystack) {
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                    return WalkState::Quit;
                }
                // Keep what was printed before the error (ripgrep 16, #3382).
                Err(_) => {}
            }

            let buf = searcher.printer().get_mut().get_ref();
            // Only include results for valid UTF-8 output
            if !buf.is_empty()
                && let Ok(results_str) = String::from_utf8(buf.clone())
                && let Ok(mut guard) = results.lock()
            {
                guard.push(results_str);
            }

            WalkState::Continue
        })
    });

    Ok(results.into_inner().unwrap())
}

/// Search one haystack, returning grep-printer's JSON messages for it (None when nothing
/// was printed or the output is not UTF-8).
fn search_haystack_json(
    args: &HiArgs,
    matcher: &search::PatternMatcher,
    searcher: &mut grep::searcher::Searcher,
    haystack: &haystack::Haystack,
) -> std::io::Result<Option<String>> {
    let mut printer_buf: Vec<u8> = vec![];
    searcher.set_binary_detection(args.binary_detection(haystack));
    {
        let mut json_printer = args.printer_json(&mut printer_buf);
        let path = haystack.path();
        match matcher {
            search::PatternMatcher::RustRegex(m) => {
                let mut sink = json_printer.sink_with_path(m, path);
                searcher.search_path(m, path, &mut sink)?;
            }
        }
    }
    if printer_buf.is_empty() {
        return Ok(None);
    }
    Ok(String::from_utf8(printer_buf).ok())
}

/// Search one haystack, collecting at most `limit` matches.
fn search_haystack_structured(
    args: &HiArgs,
    matcher: &search::PatternMatcher,
    searcher: &mut grep::searcher::Searcher,
    haystack: &haystack::Haystack,
    limit: Option<usize>,
) -> std::io::Result<Vec<SearchMatch>> {
    searcher.set_binary_detection(args.binary_detection(haystack));
    let path = haystack.path();
    let path_str = path.to_string_lossy();
    match matcher {
        search::PatternMatcher::RustRegex(m) => {
            let mut sink = StructuredSink::new(m, &path_str, limit);
            searcher.search_path(m, path, &mut sink)?;
            Ok(sink.matches)
        }
    }
}

/// JSON mode search implementation
fn py_search_impl_json(args: &HiArgs) -> anyhow::Result<Vec<String>> {
    if args.threads() > 1 && args.sort_mode().is_none() {
        return py_search_impl_json_parallel(args);
    }

    let haystack_builder = args.haystack_builder();
    let unsorted = args
        .walk_builder()?
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));
    let haystacks = args.sort(unsorted);

    let matcher = args.matcher()?;
    let mut searcher = args.searcher()?;

    let mut results = Vec::new();

    for haystack in haystacks {
        match search_haystack_json(args, &matcher, &mut searcher, &haystack) {
            Ok(output) => results.extend(output),
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => break,
            Err(err) => {
                err_message!("{}: {}", haystack.path().display(), err);
            }
        }
    }

    Ok(results)
}

/// Parallel JSON mode search (unsorted: results come in walk order).
fn py_search_impl_json_parallel(args: &HiArgs) -> anyhow::Result<Vec<String>> {
    use std::sync::Mutex;

    let haystack_builder = args.haystack_builder();
    let matcher = args.matcher()?;
    let searcher = args.searcher()?;
    let results = Mutex::new(Vec::new());

    args.walk_builder()?.build_parallel().run(|| {
        let haystack_builder = &haystack_builder;
        let matcher = &matcher;
        let results = &results;
        let mut searcher = searcher.clone();

        Box::new(move |result| {
            let Some(haystack) = haystack_builder.build_from_result(result) else {
                return WalkState::Continue;
            };
            match search_haystack_json(args, matcher, &mut searcher, &haystack) {
                Ok(Some(output)) => {
                    if let Ok(mut guard) = results.lock() {
                        guard.push(output);
                    }
                }
                Ok(None) => {}
                Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                    return WalkState::Quit;
                }
                Err(err) => {
                    err_message!("{}: {}", haystack.path().display(), err);
                }
            }
            WalkState::Continue
        })
    });

    Ok(results.into_inner().unwrap())
}

/// Structured search implementation - returns parsed SearchMatch objects
fn py_search_impl_structured(
    args: &HiArgs,
    max_total: Option<u64>,
) -> anyhow::Result<Vec<SearchMatch>> {
    let max_total = max_total.map(|m| m as usize);
    if args.threads() > 1 && args.sort_mode().is_none() {
        return py_search_impl_structured_parallel(args, max_total);
    }

    let haystack_builder = args.haystack_builder();
    let unsorted = args
        .walk_builder()?
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));
    let haystacks = args.sort(unsorted);

    let matcher = args.matcher()?;
    let mut searcher = args.searcher()?;

    let mut results = Vec::new();

    for haystack in haystacks {
        let remaining = max_total.map(|max| max - results.len());
        if remaining == Some(0) {
            break;
        }

        match search_haystack_structured(args, &matcher, &mut searcher, &haystack, remaining) {
            Ok(matches) => results.extend(matches),
            Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => break,
            Err(err) => {
                err_message!("{}: {}", haystack.path().display(), err);
            }
        }
    }

    Ok(results)
}

/// Parallel structured search (unsorted: results come in walk order). `max_total` is
/// enforced with a shared counter; workers racing past it are truncated at the end.
fn py_search_impl_structured_parallel(
    args: &HiArgs,
    max_total: Option<usize>,
) -> anyhow::Result<Vec<SearchMatch>> {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let haystack_builder = args.haystack_builder();
    let matcher = args.matcher()?;
    let searcher = args.searcher()?;
    let results = Mutex::new(Vec::new());
    let collected = AtomicUsize::new(0);

    args.walk_builder()?.build_parallel().run(|| {
        let haystack_builder = &haystack_builder;
        let matcher = &matcher;
        let results = &results;
        let collected = &collected;
        let mut searcher = searcher.clone();

        Box::new(move |result| {
            let remaining = match max_total {
                Some(max) => match max.checked_sub(collected.load(Ordering::Relaxed)) {
                    Some(0) | None => return WalkState::Quit,
                    remaining => remaining,
                },
                None => None,
            };
            let Some(haystack) = haystack_builder.build_from_result(result) else {
                return WalkState::Continue;
            };
            match search_haystack_structured(args, matcher, &mut searcher, &haystack, remaining) {
                Ok(matches) if !matches.is_empty() => {
                    let count = matches.len();
                    if let Ok(mut guard) = results.lock() {
                        guard.extend(matches);
                    }
                    let total = collected.fetch_add(count, Ordering::Relaxed) + count;
                    if max_total.is_some_and(|max| total >= max) {
                        return WalkState::Quit;
                    }
                }
                Ok(_) => {}
                Err(err) if err.kind() == std::io::ErrorKind::BrokenPipe => {
                    return WalkState::Quit;
                }
                Err(err) => {
                    err_message!("{}: {}", haystack.path().display(), err);
                }
            }
            WalkState::Continue
        })
    });

    let mut results = results.into_inner().unwrap();
    if let Some(max) = max_total {
        results.truncate(max);
    }
    Ok(results)
}

#[pyfunction]
#[pyo3(name = "search_structured")]
#[pyo3(signature = (
    patterns,
    paths=None,
    globs=None,
    glob_not=None,
    sort=None,
    max_count=None,
    max_bytes=None,
    max_total=None,
    multiline=None,
    case_sensitive=None,
    smart_case=None,
    no_ignore=None,
    hidden=None,
    follow=false,
))]
pub fn py_search_structured(
    py: Python<'_>,
    patterns: Vec<String>,
    paths: Option<Vec<String>>,
    globs: Option<Vec<String>>,
    glob_not: Option<Vec<String>>,
    sort: Option<PySortMode>,
    max_count: Option<u64>,
    max_bytes: Option<u64>,
    max_total: Option<u64>,
    multiline: Option<bool>,
    case_sensitive: Option<bool>,
    smart_case: Option<bool>,
    no_ignore: Option<bool>,
    hidden: Option<bool>,
    follow: bool,
) -> PyResult<Vec<SearchMatch>> {
    py.detach(|| {
        let py_args = PyArgs {
            patterns,
            paths,
            globs,
            glob_not,
            heading: None,
            after_context: None,
            before_context: None,
            separator_field_context: None,
            separator_field_match: None,
            separator_context: None,
            sort,
            max_count,
            max_bytes,
            line_number: None,
            multiline,
            case_sensitive,
            smart_case,
            no_ignore,
            hidden,
            json: Some(true),
            include_dirs: None,
            max_depth: None,
            absolute: None,
            relative_to: None,
            follow,
        };

        let mode = lowargs::Mode::Search(lowargs::SearchMode::JSON);
        let args = pyargs_to_hiargs(&py_args, mode)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;

        py_search_impl_structured(&args, max_total)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    })
}

#[pyfunction]
#[pyo3(name = "files")]
#[pyo3(signature = (
    paths=None,
    globs=None,
    glob_not=None,
    sort=None,
    max_count=None,
    no_ignore=None,
    hidden=None,
    include_dirs=None,
    max_depth=None,
    absolute=None,
    relative_to=None,
    follow=false,
))]
pub fn py_files(
    py: Python<'_>,
    paths: Option<Vec<String>>,
    globs: Option<Vec<String>>,
    glob_not: Option<Vec<String>>,
    sort: Option<PySortMode>,
    max_count: Option<u64>,
    no_ignore: Option<bool>,
    hidden: Option<bool>,
    include_dirs: Option<bool>,
    max_depth: Option<usize>,
    absolute: Option<bool>,
    relative_to: Option<String>,
    follow: bool,
) -> PyResult<Vec<String>> {
    py.detach(|| {
        let py_args = PyArgs {
            patterns: vec![],
            paths,
            globs,
            glob_not,
            heading: None,
            after_context: None,
            before_context: None,
            separator_field_context: None,
            separator_field_match: None,
            separator_context: None,
            sort,
            max_count,
            max_bytes: None,
            line_number: None,
            multiline: None,
            case_sensitive: None,
            smart_case: None,
            no_ignore,
            hidden,
            json: None,
            include_dirs,
            max_depth,
            absolute,
            relative_to,
            follow,
        };

        let args_result = pyargs_to_hiargs(&py_args, lowargs::Mode::Files);

        if let Err(err) = args_result {
            return Err(PyValueError::new_err(err.to_string()));
        }

        let args = args_result.unwrap();

        let roots = py_args.paths.clone().unwrap_or_default();
        let files_result = py_files_impl(
            &args,
            py_args.absolute.unwrap_or(false),
            py_args.relative_to.as_deref(),
            &roots,
        );

        if let Err(err) = files_result {
            return Err(PyValueError::new_err(err.to_string()));
        }

        Ok(files_result.unwrap())
    })
}

/// Turns walked paths into the strings `files` / `files_with_info` return: made absolute
/// (lexically: symlinks are not resolved, so paths stay under the root that was asked
/// for), `relative_to` stripped, the root directories themselves dropped.
struct PathFormat {
    /// The working directory, set when absolute paths were requested.
    cwd: Option<PathBuf>,
    prefix: Option<String>,
    roots: Vec<String>,
}

impl PathFormat {
    fn new(absolute: bool, relative_to: Option<&str>, roots: &[String]) -> Self {
        let cwd = absolute.then(|| std::env::current_dir().unwrap_or_default());
        let mut format = PathFormat {
            cwd,
            prefix: None,
            roots: Vec::new(),
        };
        // In absolute mode, the prefix and roots go through the same normalization as the
        // walked paths, so they compare equal.
        let normalize = |p: &str| {
            let p = normalize_separators(p);
            format.normalize(Path::new(&p)).unwrap_or(p)
        };
        let prefix = relative_to.map(|p| {
            let mut s = normalize(p);
            if !s.ends_with(std::path::MAIN_SEPARATOR) {
                s.push(std::path::MAIN_SEPARATOR);
            }
            s
        });
        let roots = roots
            .iter()
            .map(|root| normalize(root).trim_end_matches(['/', '\\']).to_string())
            .collect();
        format.prefix = prefix;
        format.roots = roots;
        format
    }

    /// Make a path absolute (if requested) without touching the file system.
    fn normalize(&self, path: &Path) -> Option<String> {
        let Some(cwd) = &self.cwd else {
            return path.to_str().map(str::to_string);
        };
        let full_path = if path.is_absolute() {
            Cow::Borrowed(path)
        } else {
            Cow::Owned(cwd.join(path))
        };
        // Rebuilding from components also turns `/` into `\` on Windows.
        let needs_cleanup = cfg!(windows)
            || full_path
                .components()
                .any(|c| matches!(c, Component::ParentDir | Component::CurDir));
        let full_path = if needs_cleanup {
            let mut normalized = PathBuf::new();
            for component in full_path.components() {
                match component {
                    Component::ParentDir => {
                        normalized.pop();
                    }
                    Component::CurDir => {}
                    _ => normalized.push(component),
                }
            }
            Cow::Owned(normalized)
        } else {
            full_path
        };
        let s = full_path.to_str()?;
        Some(s.strip_prefix(r"\\?\").unwrap_or(s).to_string())
    }

    /// Returns None for the root directory itself (empty, ".", or matching a root path).
    fn apply(&self, path: &Path) -> Option<String> {
        let mut path_str = self.normalize(path)?;
        if let Some(pfx) = &self.prefix {
            if path_str == pfx.trim_end_matches(['/', '\\']) {
                // This is the root directory itself — skip it
                return None;
            }
            if path_str.starts_with(pfx.as_str()) {
                path_str = path_str[pfx.len()..].to_string();
            }
        }
        if path_str.is_empty() || path_str == "." {
            return None;
        }
        // Filter out root search paths themselves
        if self.roots.contains(&path_str) {
            return None;
        }
        Some(path_str)
    }
}

/// Normalize path separators to the platform default.
/// On Windows, converts `/` to `\`. On Unix, this is a no-op.
fn normalize_separators(path: &str) -> String {
    if cfg!(windows) {
        path.replace('/', "\\")
    } else {
        path.to_string()
    }
}

fn py_files_impl(
    args: &HiArgs,
    absolute: bool,
    relative_to: Option<&str>,
    roots: &[String],
) -> anyhow::Result<Vec<String>> {
    // Use parallel implementation when threads > 1 and no sorting requested
    if args.threads() > 1 && args.sort_mode().is_none() {
        return py_files_impl_parallel(args, absolute, relative_to, roots);
    }

    let format = PathFormat::new(absolute, relative_to, roots);

    let haystack_builder = args.haystack_builder();
    let unsorted = args
        .walk_builder()?
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));
    let haystacks = args.sort(unsorted);

    let mut matches = Vec::new();

    for haystack in haystacks {
        if args.quit_after_match() {
            break;
        }

        if let Some(max_count) = args.max_count()
            && matches.len() >= max_count as usize
        {
            break;
        }

        if let Some(p) = format.apply(haystack.path()) {
            matches.push(p);
        }
    }

    Ok(matches)
}
/// Parallel file listing implementation
fn py_files_impl_parallel(
    args: &HiArgs,
    absolute: bool,
    relative_to: Option<&str>,
    roots: &[String],
) -> anyhow::Result<Vec<String>> {
    let haystack_builder = args.haystack_builder();
    let max_count = args.max_count();
    let quit_after_match = args.quit_after_match();
    let format = PathFormat::new(absolute, relative_to, roots);

    let (tx, rx) = mpsc::channel::<String>();

    // Spawn collector thread
    let collector = thread::spawn(move || -> Vec<String> {
        let mut results = Vec::new();
        for path in rx.iter() {
            results.push(path);
            if quit_after_match {
                break;
            }
            if let Some(max) = max_count
                && results.len() >= max as usize
            {
                break;
            }
        }
        results
    });

    // Parallel directory walk
    args.walk_builder()?.build_parallel().run(|| {
        let haystack_builder = &haystack_builder;
        let tx = tx.clone();
        let format = &format;

        Box::new(move |result| {
            let haystack = match haystack_builder.build_from_result(result) {
                Some(haystack) => haystack,
                None => return WalkState::Continue,
            };

            if let Some(p) = format.apply(haystack.path()) {
                match tx.send(p) {
                    Ok(_) => WalkState::Continue,
                    Err(_) => WalkState::Quit,
                }
            } else {
                WalkState::Continue
            }
        })
    });

    // Drop the original sender so collector thread can finish
    drop(tx);

    // Collect results
    Ok(collector.join().unwrap())
}

/// Helper to create FileInfo from path and metadata
fn create_file_info(path_str: String, metadata: &std::fs::Metadata, islink: bool) -> FileInfo {
    let mtime = metadata
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);

    let created = metadata
        .created()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs_f64())
        .unwrap_or(mtime); // Fall back to mtime if created not available

    let file_type = if metadata.is_dir() {
        "directory".to_string()
    } else {
        "file".to_string()
    };

    #[cfg(unix)]
    let (mode, uid, gid, ino, nlink) = {
        use std::os::unix::fs::MetadataExt;
        (
            metadata.mode(),
            metadata.uid(),
            metadata.gid(),
            metadata.ino(),
            metadata.nlink(),
        )
    };

    #[cfg(not(unix))]
    let (mode, uid, gid, ino, nlink) = {
        // On Windows, we don't have these Unix-specific fields
        let mode = if metadata.permissions().readonly() {
            0o444
        } else {
            0o644
        };
        (mode, 0u32, 0u32, 0u64, 1u64)
    };

    FileInfo {
        name: path_str,
        size: metadata.len(),
        file_type,
        created,
        islink,
        mode,
        uid,
        gid,
        mtime,
        ino,
        nlink,
    }
}

#[pyfunction]
#[pyo3(name = "files_with_info")]
#[pyo3(signature = (
    paths=None,
    globs=None,
    glob_not=None,
    sort=None,
    no_ignore=None,
    hidden=None,
    include_dirs=None,
    max_depth=None,
    absolute=None,
    relative_to=None,
    follow=false,
))]
pub fn py_files_with_info(
    py: Python<'_>,
    paths: Option<Vec<String>>,
    globs: Option<Vec<String>>,
    glob_not: Option<Vec<String>>,
    sort: Option<PySortMode>,
    no_ignore: Option<bool>,
    hidden: Option<bool>,
    include_dirs: Option<bool>,
    max_depth: Option<usize>,
    absolute: Option<bool>,
    relative_to: Option<String>,
    follow: bool,
) -> PyResult<HashMap<String, FileInfo>> {
    py.detach(|| {
        let py_args = PyArgs {
            patterns: vec![],
            paths,
            globs,
            glob_not,
            heading: None,
            after_context: None,
            before_context: None,
            separator_field_context: None,
            separator_field_match: None,
            separator_context: None,
            sort,
            max_count: None,
            max_bytes: None,
            line_number: None,
            multiline: None,
            case_sensitive: None,
            smart_case: None,
            no_ignore,
            hidden,
            json: None,
            include_dirs,
            max_depth,
            absolute,
            relative_to,
            follow,
        };

        let args = pyargs_to_hiargs(&py_args, lowargs::Mode::Files)
            .map_err(|err| PyValueError::new_err(err.to_string()))?;
        let absolute = py_args.absolute.unwrap_or(false);
        let roots = py_args.paths.clone().unwrap_or_default();

        py_files_with_info_impl(&args, absolute, py_args.relative_to.as_deref(), &roots)
            .map_err(|err| PyValueError::new_err(err.to_string()))
    })
}

fn py_files_with_info_impl(
    args: &HiArgs,
    absolute: bool,
    relative_to: Option<&str>,
    roots: &[String],
) -> anyhow::Result<HashMap<String, FileInfo>> {
    // Use parallel implementation when threads > 1 and no sorting requested
    if args.threads() > 1 && args.sort_mode().is_none() {
        return py_files_with_info_impl_parallel(args, absolute, relative_to, roots);
    }

    let format = PathFormat::new(absolute, relative_to, roots);

    // Single-threaded implementation
    let haystack_builder = args.haystack_builder();
    let walk_builder = args.walk_builder()?;

    let unsorted = walk_builder
        .build()
        .filter_map(|result| haystack_builder.build_from_result(result));

    let haystacks = args.sort(unsorted);

    let mut results = HashMap::new();

    for haystack in haystacks {
        if args.quit_after_match() {
            break;
        }

        if let Some(max_count) = args.max_count()
            && results.len() >= max_count as usize
        {
            break;
        }

        let path = haystack.path();
        if let Some(path_str) = format.apply(path)
            && let Ok(metadata) = std::fs::metadata(path)
        {
            let islink = std::fs::symlink_metadata(path).is_ok_and(|m| m.is_symlink());
            let info = create_file_info(path_str.clone(), &metadata, islink);
            results.insert(path_str, info);
        }
    }

    Ok(results)
}

/// Parallel file listing with info implementation
fn py_files_with_info_impl_parallel(
    args: &HiArgs,
    absolute: bool,
    relative_to: Option<&str>,
    roots: &[String],
) -> anyhow::Result<HashMap<String, FileInfo>> {
    let haystack_builder = args.haystack_builder();
    let max_count = args.max_count();
    let quit_after_match = args.quit_after_match();
    let format = PathFormat::new(absolute, relative_to, roots);

    let (tx, rx) = mpsc::channel::<(String, FileInfo)>();

    // Spawn collector thread
    let collector = thread::spawn(move || -> HashMap<String, FileInfo> {
        let mut results = HashMap::new();
        for (path, info) in rx.iter() {
            results.insert(path, info);
            if quit_after_match {
                break;
            }
            if let Some(max) = max_count
                && results.len() >= max as usize
            {
                break;
            }
        }
        results
    });

    // Parallel directory walk
    args.walk_builder()?.build_parallel().run(|| {
        let haystack_builder = &haystack_builder;
        let tx = tx.clone();
        let format = &format;

        Box::new(move |result| {
            let haystack = match haystack_builder.build_from_result(result) {
                Some(haystack) => haystack,
                None => return WalkState::Continue,
            };

            let path = haystack.path();
            if let Some(path_str) = format.apply(path)
                && let Ok(metadata) = std::fs::metadata(path)
            {
                let islink = std::fs::symlink_metadata(path).is_ok_and(|m| m.is_symlink());
                let info = create_file_info(path_str.clone(), &metadata, islink);
                match tx.send((path_str, info)) {
                    Ok(_) => return WalkState::Continue,
                    Err(_) => return WalkState::Quit,
                }
            }

            WalkState::Continue
        })
    });

    // Drop the original sender so collector thread can finish
    drop(tx);

    // Collect results
    Ok(collector.join().unwrap())
}
