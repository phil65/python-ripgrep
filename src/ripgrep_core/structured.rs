/*!
A searcher sink that collects matches directly as `SearchMatch` values.

Submatch discovery mirrors grep-printer's JSON sink (`find_iter_at_in_context`
and `record_matches`) so results are identical to its `match` messages, minus
the serialize/parse round-trip. Non-UTF-8 bytes are decoded lossily.
*/

use std::io;

use grep::matcher::Matcher;
use grep::searcher::{Searcher, Sink, SinkError, SinkMatch};

use super::{SearchMatch, SearchSubmatch};

/// Same cap grep-printer uses when re-finding matches in multi-line mode, to
/// allow look-ahead past the matched lines without scanning the whole buffer.
const MAX_LOOK_AHEAD: usize = 128;

pub(crate) struct StructuredSink<'a, M> {
    matcher: &'a M,
    path: &'a str,
    limit: Option<usize>,
    pub(crate) matches: Vec<SearchMatch>,
}

impl<'a, M: Matcher> StructuredSink<'a, M> {
    /// `limit` stops the search once that many matches were collected.
    pub(crate) fn new(matcher: &'a M, path: &'a str, limit: Option<usize>) -> Self {
        StructuredSink {
            matcher,
            path,
            limit,
            matches: Vec::new(),
        }
    }

    fn submatches(
        &self,
        searcher: &Searcher,
        mat: &SinkMatch<'_>,
    ) -> io::Result<Vec<SearchSubmatch>> {
        let buf = mat.buffer();
        let range = mat.bytes_range_in_buffer();
        let lines = mat.bytes();

        let haystack = if searcher.multi_line_with_matcher(self.matcher) {
            &buf[..buf.len().min(range.end + MAX_LOOK_AHEAD)]
        } else {
            // Exclude the line terminator so look-around can't observe it.
            &buf[..end_without_terminator(searcher, buf, range.end)]
        };

        let mut spans = Vec::new();
        self.matcher
            .find_iter_at(haystack, range.start, |m| {
                if m.start() >= range.end {
                    return false;
                }
                spans.push((m.start() - range.start, m.end() - range.start));
                true
            })
            .map_err(io::Error::error_message)?;
        if let Some(&(start, end)) = spans.last()
            && start == end
            && start >= lines.len()
        {
            spans.pop();
        }

        Ok(spans
            .into_iter()
            .map(|(start, end)| SearchSubmatch {
                text: String::from_utf8_lossy(&lines[start..end]).into_owned(),
                start,
                end,
            })
            .collect())
    }
}

fn end_without_terminator(searcher: &Searcher, buf: &[u8], end: usize) -> usize {
    let lineterm = searcher.line_terminator();
    if !lineterm.is_suffix(&buf[..end]) {
        return end;
    }
    let mut end = end - 1;
    if lineterm.is_crlf() && end > 0 && buf[end - 1] == b'\r' {
        end -= 1;
    }
    end
}

impl<M: Matcher> Sink for StructuredSink<'_, M> {
    type Error = io::Error;

    fn matched(&mut self, searcher: &Searcher, mat: &SinkMatch<'_>) -> io::Result<bool> {
        let submatches = self.submatches(searcher, mat)?;
        let line_text = String::from_utf8_lossy(mat.bytes());
        self.matches.push(SearchMatch {
            path: self.path.to_owned(),
            line_number: mat.line_number().unwrap_or(0),
            absolute_offset: mat.absolute_byte_offset(),
            line_text: line_text.trim_end_matches('\n').to_owned(),
            submatches,
        });
        Ok(self.limit.is_none_or(|limit| self.matches.len() < limit))
    }
}
