use std::{cmp::{max, min}};
use std::fmt;

mod range;
pub(crate) use range::Range;

/// A set of ranges.
#[derive(Clone, Default, Debug, PartialEq)]
pub struct RangeSet {
    /// For the implementation it is assumed that `ranges` is ordered by `range.start()`.
    ranges: Vec<Range>,
}

impl fmt::Display for RangeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "(")?;
        for range in &self.ranges {
            write!(f, "{}", range)?;
        }
        write!(f, ")")
    }
}

impl std::ops::Index<usize> for RangeSet {
    type Output = Range;

    fn index(&self, index: usize) -> &Self::Output {
        &self.ranges[index]
    }
}

impl std::ops::IndexMut<usize> for RangeSet {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        &mut self.ranges[index]
    }
}

impl IntoIterator for RangeSet {
    type Item = Range;

    type IntoIter = <Vec<Range> as IntoIterator>::IntoIter;

    fn into_iter(self) -> Self::IntoIter {
        self.ranges.into_iter()
    }
}

impl From<Vec<Range>> for RangeSet {
    fn from(ranges: Vec<Range>) -> Self {
        let mut res = RangeSet::new();
        for range in ranges {
            res += range;
        }
        res
    }
}

impl RangeSet {
    pub fn new() -> RangeSet {
        Self::default()
    }

    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }

    pub fn len(&self) -> usize {
        self.ranges.iter().map(Range::len).sum()
    }

    pub fn iter(&self) -> impl Iterator<Item=&Range> {
        self.ranges.iter()
    }

    pub fn contains(&self, value: usize) -> bool {
        self
            .iter()
            .any(|r| r.contains(value))
    }

    /// Finds the first `Range` in `self.ranges` which contains `value`, and returns the number of
    /// elements that are left in that `Range`.
    pub fn contained_length_from_value(&self, value: usize) -> usize {
        self
            .iter()
            .take_while(|r| value >= r.start()) // stop once the ranges no longer contain this value
            .find(|r| r.contains(value)) // find the first range that contains this value
            .map(|r| r.end() - value) // calculate the remaining elements in that range
            .unwrap_or(0) // return zero otherwise
    }

    #[allow(dead_code)]
    pub fn contains_range_set(&self, other: &RangeSet) -> bool {
        other.iter().all(|r| self.contained_length_from_value(r.start()) >= r.len())
    }
}

impl std::ops::AddAssign<Range> for RangeSet {
    fn add_assign(&mut self, range: Range) {
        if range.is_empty() {
            // the interval is empty -> nothing to do.
            return;
        }

        for index in 0..self.ranges.len() {
            let mut cur = self[index];
            // the new range is clear of any ranges we already iterated over.
            if range.end() < cur.start() {
                // the new range starts after anything we already passed and ends before the next
                // range starts (they don't touch) -> insert it.
                self.ranges.insert(index, range);
                return
            } else if range.start() <= cur.end() && cur.start() <= range.end() {
                // the new range overlaps (or touches) the first range. They are to be merged.
                // In addition we might have to merge further ranges in as well.

                let mut new_range = range;
                while cur.start() <= new_range.end() {
                    let new_end = max(new_range.end(), cur.end());
                    let new_start = min(new_range.start(), cur.start());
                    new_range = (new_start..new_end).into();
                    self.ranges.remove(index);
                    if index >= self.ranges.len() {
                        break;
                    }
                    cur = self[index];
                }

                self.ranges.insert(index, new_range);
                return
            }
        }

        // the new range is after everything else -> just add it
        self.ranges.push(range);
    }
}

impl std::ops::Add<Range> for RangeSet {
    type Output = RangeSet;

    fn add(mut self, rhs: Range) -> Self::Output {
        // let mut cpy = self.clone();
        self += rhs;
        self
    }
}

impl std::ops::AddAssign<&RangeSet> for RangeSet {
    fn add_assign(&mut self, rhs: &RangeSet) {
        for &range in rhs.iter() {
            *self += range;
        }
    }
}

impl std::ops::Add<&RangeSet> for RangeSet {
    type Output = RangeSet;

    fn add(mut self, rhs: &RangeSet) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::Add<&RangeSet> for &RangeSet {
    type Output = RangeSet;

    fn add(self, rhs: &RangeSet) -> Self::Output {
        let mut cpy = self.clone();
        cpy += rhs;
        cpy
    }
}

impl std::ops::SubAssign<Range> for RangeSet {
    fn sub_assign(&mut self, to_sub: Range) {
        if to_sub.len() == 0 {
            return;
        }

        for index in 0..self.ranges.len() {
            let cur = self[index];
            // the ranges we already passed don't overlap with the range to remove
            if to_sub.end() <= cur.start() {
                // the remaining ranges are past the one to subtract. -> we're done.
                return;
            }
            if to_sub.start() <= cur.start() && cur.start() < to_sub.end() {
                // the range to subtract started before the current range and reaches into the
                // current range -> we have to remove the beginning of the range or the entire range
                // and do the same for following ranges.

                while index < self.ranges.len() && self[index].end() <= to_sub.end() {
                    self.ranges.remove(index);
                }

                if index < self.ranges.len() && self[index].start() < to_sub.end() {
                    self[index] = (to_sub.end()..self[index].end()).into();
                }
                return;
            } else if to_sub.end() < cur.end() {
                // the range to subtract punches a hole into the current range -> we need to create
                // two smaller ranges.
                let first_range = (cur.start()..to_sub.start()).into();
                self[index] = (to_sub.end()..cur.end()).into();
                self.ranges.insert(index, first_range);
                return;
            } else if to_sub.start() < cur.end() {
                // the range truncates the existing range -> truncate the range. Let the for loop 
                // take care of overlaps with other ranges.
                self[index] = (self[index].start()..to_sub.start()).into();
            }
        }
    }
}

impl std::ops::Sub<Range> for RangeSet {
    type Output = RangeSet;

    fn sub(self, rhs: Range) -> Self::Output {
        let mut cpy = self.clone();
        cpy -= rhs;
        cpy
        
    }
}

impl std::ops::SubAssign<&RangeSet> for RangeSet {
    fn sub_assign(&mut self, rhs: &RangeSet) {
        for &range in rhs.iter() {
            *self -= range;
        }
    }
}

impl std::ops::Sub<&RangeSet> for RangeSet {
    type Output = RangeSet;

    fn sub(mut self, rhs: &RangeSet) -> Self::Output {
        self += rhs;
        self
    }
}

impl std::ops::Sub<&RangeSet> for &RangeSet {
    type Output = RangeSet;

    fn sub(self, rhs: &RangeSet) -> Self::Output {
        let mut cpy = self.clone();
        cpy -= rhs;
        cpy
    }
}

impl std::ops::BitAnd<&RangeSet> for RangeSet {
    type Output = RangeSet;

    fn bitand(self, other: &RangeSet) -> Self::Output {
        let mut result = RangeSet::new();

        let mut self_index: usize = 0;
        let mut other_index: usize = 0;

        while self_index < self.ranges.len() && other_index < other.ranges.len() {
            let self_range = self[self_index];
            let other_range = other[other_index];
            if self_range.end() <= other_range.start() {
                // skip the interval
                self_index += 1;
            } else if other_range.end() <= self_range.start() {
                // skip the interval
                other_index += 1;
            } else {
                // the two intervals overlap. Add the union and advance the index of the one that ends first.
                let new_start = max(self_range.start(), other_range.start());
                let new_end = min(self_range.end(), other_range.end());
                let new_range: Range = (new_start..new_end).into();
                result += new_range;
                if self_range.end() <= other_range.end() {
                    self_index += 1;
                } else {
                    other_index += 1;
                }
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_set1() -> RangeSet {
        vec![
            (0..10).into(),
            (20..30).into(),
        ].into()
    }

    fn test_set2() -> RangeSet {
        vec![
            (0..20).into(),
            (20..30).into(),
        ].into()
    }

    fn test_set3() -> RangeSet {
        vec![
            (0..10).into(),
            (10..20).into(),
            (20..30).into(),
        ].into()
    }

    fn test_set4() -> RangeSet {
        vec![
            (20..30).into(),
        ].into()
    }

    fn is_sorted(rs: &RangeSet) -> bool {
        let first = rs[0];
        rs
            .ranges[1..]
            .iter()
            .fold((true, first), |(is_sorted, prev), &r| (is_sorted && prev.start() <= r.start(), r))
            .0
    }

    #[test]
    fn test_is_empty() {
        let empty_set: RangeSet = Vec::new().into();
        assert!(empty_set.is_empty());
    }

    #[test]
    fn test_len() {
        assert_eq!(test_set1().len(), 20);
        assert_eq!(test_set2().len(), 30);
        assert_eq!(test_set3().len(), 30);
    }

    #[test]
    fn test_contains() {
        assert!(test_set1().contains(0));
        assert!(test_set1().contains(5));
        assert!(!test_set1().contains(10));

        assert!(test_set2().contains(0));
        assert!(test_set2().contains(20));
        assert!(!test_set2().contains(30));

        assert!(test_set3().contains(0));
        assert!(test_set3().contains(5));
        assert!(test_set3().contains(10));
    }

    #[test]
    fn contained_length_from_value() {
        assert_eq!(test_set1().contained_length_from_value(0), 10);
        assert_eq!(test_set1().contained_length_from_value(10), 0);
        assert_eq!(test_set1().contained_length_from_value(20), 10);
        
        // todo: should these tests pass?
        // assert_eq!(test_set2().contained_length_from_value(0), 30);
        // assert_eq!(test_set2().contained_length_from_value(30), 0);
        
        // assert_eq!(test_set3().contained_length_from_value(0), 30);
        // assert_eq!(test_set3().contained_length_from_value(10), 20);
        // assert_eq!(test_set3().contained_length_from_value(20), 10);
        // assert_eq!(test_set3().contained_length_from_value(30), 10);
    }

    #[test]
    fn test_contains_range_set() {
        assert!(test_set1().contains_range_set(&test_set1()));
        assert!(!test_set1().contains_range_set(&test_set2()));
        assert!(!test_set1().contains_range_set(&test_set3()));

        assert!(test_set2().contains_range_set(&test_set1()));
        assert!(test_set2().contains_range_set(&test_set2()));
        assert!(test_set2().contains_range_set(&test_set3()));

        assert!(test_set3().contains_range_set(&test_set1()));
        assert!(test_set3().contains_range_set(&test_set2()));
        assert!(test_set3().contains_range_set(&test_set3()));
    }

    #[test]
    fn test_add() {
        let mut test1 = test_set1();
        let to_add: Range = (0..10).into();
        test1 += to_add;
        assert_eq!(test_set1(), test1);
        let to_add: Range = (10..20).into();
        test1 += to_add;
        assert_eq!(test_set2(), test1);
        let to_add: Range = (5..15).into();
        test1 += to_add;
        assert_eq!(test_set2(), test1);

        assert!(is_sorted(&test1));
    }

    #[test]
    fn test_sub() {
        let mut test1 = test_set1();
        let to_sub: Range = (0..10).into();
        test1 -= to_sub;
        assert_eq!(test_set4(), test1);

        let mut test3 = test_set3();
        let to_sub: Range = (10..20).into();
        test3 -= to_sub;
        assert_eq!(test_set1(), test3);

        let mut test4: RangeSet = vec![(0..221184).into()].into();
        let to_sub: Range = (0..69632).into();
        test4 -= to_sub;
        let res: RangeSet = vec![(69632..221184).into()].into();
        assert_eq!(res, test4);
    }
}
