//! Line splitting and prefiltering shared by JSON Lines connectors.
//!
//! Logs can hold gigabytes of tool output, so both run at memory speed (SIMD `memchr`)
//! and only lines that can carry a record reach the JSON parser.

use memchr::memmem::Finder;

/// Complete lines of `data` with their absolute byte offsets. A trailing `\r` is removed
/// and blank lines are skipped. The caller passes only complete lines.
pub fn with_offsets(data: &[u8], base_offset: u64) -> impl Iterator<Item = (u64, &[u8])> {
    let mut start = 0;
    let ends =
        memchr::memchr_iter(b'\n', data).chain((!data.ends_with(b"\n")).then_some(data.len()));
    ends.filter_map(move |end| {
        let line = &data[start..end];
        let offset = base_offset + start as u64;
        start = end + 1;
        let line = line.strip_suffix(b"\r").unwrap_or(line);
        (!line.iter().all(u8::is_ascii_whitespace)).then_some((offset, line))
    })
}

/// An exact prefilter: whether a line contains any of a fixed set of byte strings.
pub struct AnyOf<const N: usize>([Finder<'static>; N]);

impl<const N: usize> AnyOf<N> {
    pub fn new(needles: [&'static [u8]; N]) -> Self {
        Self(needles.map(Finder::new))
    }

    pub fn matches(&self, line: &[u8]) -> bool {
        self.0.iter().any(|finder| finder.find(line).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::{AnyOf, with_offsets};

    #[test]
    fn yields_lines_with_absolute_offsets() {
        let data = b"{\"a\":1}\n\n{\"b\":2}\r\n";
        let lines: Vec<_> = with_offsets(data, 100).collect();
        assert_eq!(
            lines,
            vec![(100, &b"{\"a\":1}"[..]), (109, &b"{\"b\":2}"[..])]
        );
    }

    #[test]
    fn yields_a_final_line_without_newline() {
        let lines: Vec<_> = with_offsets(b"a\nbc", 0).collect();
        assert_eq!(lines, vec![(0, &b"a"[..]), (2, &b"bc"[..])]);
        assert_eq!(with_offsets(b"", 0).count(), 0);
    }

    #[test]
    fn finds_any_needle() {
        let filter = AnyOf::new([b"\"usage\"", b"\"tool_result\""]);
        assert!(filter.matches(b"abc\"usage\"def"));
        assert!(filter.matches(b"{\"type\":\"tool_result\"}"));
        assert!(!filter.matches(b"usage without quotes"));
    }
}
