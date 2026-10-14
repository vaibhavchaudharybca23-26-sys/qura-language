use std::fmt;

/// Represents a contiguous span of byte offsets within a source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    #[inline]
    pub const fn new(start: u32, end: u32) -> Self {
        Self { start, end }
    }

    #[inline]
    pub const fn empty(offset: u32) -> Self {
        Self {
            start: offset,
            end: offset,
        }
    }

    #[inline]
    pub fn merge(self, other: Span) -> Self {
        Self {
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    #[inline]
    pub fn len(self) -> u32 {
        self.end.saturating_sub(self.start)
    }

    #[inline]
    pub fn is_empty(self) -> bool {
        self.start >= self.end
    }
}

impl fmt::Display for Span {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }
}

/// 1-based human-readable line and column location.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

impl fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

/// Stores a source file's text and precomputes line offsets for fast location lookup.
#[derive(Debug, Clone)]
pub struct SourceFile {
    pub name: String,
    pub src: String,
    line_starts: Vec<u32>,
}

impl SourceFile {
    pub fn new(name: impl Into<String>, src: impl Into<String>) -> Self {
        let name = name.into();
        let src = src.into();
        let mut line_starts = vec![0];
        for (i, b) in src.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            name,
            src,
            line_starts,
        }
    }

    pub fn location(&self, offset: u32) -> SourceLocation {
        let offset = offset.min(self.src.len() as u32);
        let line_idx = match self.line_starts.binary_search(&offset) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };

        let line_start = self.line_starts[line_idx];
        let column = (offset - line_start) as usize + 1;
        SourceLocation {
            line: line_idx + 1,
            column,
        }
    }

    pub fn line_text(&self, line_1_indexed: usize) -> Option<&str> {
        if line_1_indexed == 0 || line_1_indexed > self.line_starts.len() {
            return None;
        }
        let start = self.line_starts[line_1_indexed - 1] as usize;
        let end = if line_1_indexed < self.line_starts.len() {
            let next_start = self.line_starts[line_1_indexed] as usize;
            if next_start > 0 && self.src.as_bytes().get(next_start - 1) == Some(&b'\n') {
                if next_start > 1 && self.src.as_bytes().get(next_start - 2) == Some(&b'\r') {
                    next_start - 2
                } else {
                    next_start - 1
                }
            } else {
                next_start
            }
        } else {
            self.src.len()
        };

        self.src.get(start..end)
    }
}
