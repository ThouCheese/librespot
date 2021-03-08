use std::fmt;

/// A struct that represents a range between two values. `std::ops::Range` is not used since it is
/// not `Copy`, which uglifies the resulting code significantly.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Range {
    start: usize,
    end: usize,
}

impl fmt::Display for Range {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}, {}]", self.start, self.end - 1)
    }
}

impl From<std::ops::Range<usize>> for Range {
    fn from(r: std::ops::Range<usize>) -> Self {
        assert!(r.start < r.end);
        Self {
            start: r.start,
            end: r.end,
        }
    }
}

impl Range {
    pub fn new(start: usize, len: usize) -> Range {
        Self {
            start,
            end: start + len,
        }
    }

    pub fn start(&self) -> usize {
        self.start
    }

    pub fn end(&self) -> usize {
        self.end
    }

    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn clear(&mut self) {
        self.end = self.start;
    }

    pub fn set_len(&mut self, len: usize) {
        self.end = self.start + len;
    }

    pub fn contains(&self, val: usize) -> bool {
        val >= self.start && val < self.end
    }
}
