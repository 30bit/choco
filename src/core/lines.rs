use super::trim;
use ::core::{iter::FusedIterator, str::Split};

#[derive(Clone, Debug)]
pub(super) struct Iter<'a> {
    lines: Split<'a, char>,
    offset: usize,
}

impl<'a> Iter<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            lines: text.split('\n'),
            offset: 0,
        }
    }

    pub(crate) fn offset(&self) -> usize {
        self.offset
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = trim::Iter<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.lines.next()?;
        // One added for new-line char
        self.offset += next.len() + 1;
        Some(trim::Iter::new(next))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.lines.size_hint()
    }
}

impl<'a> DoubleEndedIterator for Iter<'a> {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.lines.next_back().map(trim::Iter::new)
    }
}

impl<'a> FusedIterator for Iter<'a> {}
