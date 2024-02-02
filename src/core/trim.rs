use super::raw::{self, Range};
use ::core::ops;

fn remove_right(text: &str, range: ops::Range<usize>) -> ops::Range<usize> {
    match text[range.clone()].rfind(|ch: char| !ch.is_whitespace()) {
        Some(index) => {
            range.start
                ..text[range.start + index..range.end]
                    .char_indices()
                    .nth(1)
                    .map_or(range.end, |(whitespace_start, _)| {
                        range.start + index + whitespace_start
                    })
        }
        None => range.start..range.start,
    }
}

fn remove_left(text: &str, range: ops::Range<usize>) -> ops::Range<usize> {
    match text[range.clone()].find(|ch: char| !ch.is_whitespace()) {
        Some(index) => range.start + index..range.end,
        None => range.start..range.start,
    }
}

#[derive(Clone, Debug)]
pub(super) struct Iter<'a> {
    raw: raw::Iter<'a>,
    remove_left_next: bool,
    seen_signal: bool,
}

impl<'a> Iter<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            raw: raw::Iter::new(text),
            remove_left_next: true,
            seen_signal: false,
        }
    }

    pub fn as_full_str(&self) -> &'a str {
        self.raw.as_full_str()
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Range;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.raw.next()?;
        if let Range::Text(range) = &next {
            let mut range = remove_right(self.as_full_str(), range.clone());
            if self.remove_left_next {
                if self.seen_signal {
                    range = remove_left(self.as_full_str(), range);
                }
                self.remove_left_next = false;
            }
            if range.is_empty() {
                self.next()
            } else {
                Some(Range::Text(range))
            }
        } else {
            self.seen_signal = true;
            Some(next)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Iter, Range};

    #[test]
    fn no_trim_required() {
        const SAMPLE: &str = "Hello, world!";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Text(range0) = &range_event0 else {
            panic!("expected text range, got {range_event0:?}");
        };
        assert_eq!(&SAMPLE[range0.clone()], "Hello, world!");
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn middle_trim() {
        const SAMPLE: &str = "Hello, @oops world!";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Text(range0) = &range_event0 else {
            panic!("expected text range, got {range_event0:?}");
        };
        assert_eq!(&SAMPLE[range0.clone()], "Hello,");
        let range_event1 = iter.next().expect("second range event");
        let Range::Signal { .. } = &range_event1 else {
            panic!("expected signal range, got {range_event1:?}");
        };
        let range_event2 = iter.next().expect("first range event");
        let Range::Text(range2) = &range_event2 else {
            panic!("expected text range, got {range_event2:?}");
        };
        assert_eq!(&SAMPLE[range2.clone()], " world!");
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn left_trim() {
        const SAMPLE: &str = "@oops Hello, world!";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Signal { .. } = &range_event0 else {
            panic!("expected signal range, got {range_event0:?}");
        };
        let range_event1 = iter.next().expect("second range event");
        let Range::Text(range1) = &range_event1 else {
            panic!("expected text range, got {range_event1:?}");
        };
        assert_eq!(&SAMPLE[range1.clone()], "Hello, world!");
        assert_eq!(iter.next(), None);
    }
}
