use ::core::{
    iter::{FusedIterator, Peekable},
    str::CharIndices,
};

const SIGNAL_CHAR: char = '@';
const LEFT_BRACKET_CHARS: [char; 4] = ['{', '[', '(', '<'];
const RIGHT_BRACKET_CHARS: [char; 4] = ['}', ']', ')', '>'];

use ::core::ops;

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub(super) enum Range {
    Text(ops::Range<usize>),
    Signal {
        prompt: ops::Range<usize>,
        param: ops::Range<usize>,
    },
}

impl Range {
    const fn empty_signal(index: usize) -> Self {
        Self::Signal {
            prompt: index..index,
            param: index..index,
        }
    }

    const fn nameless_signal(param_range: ops::Range<usize>) -> Self {
        Self::Signal {
            prompt: param_range.start..param_range.start,
            param: param_range,
        }
    }

    const fn paramless_signal(name_range: ops::Range<usize>) -> Self {
        Self::Signal {
            param: name_range.end..name_range.end,
            prompt: name_range,
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct Iter<'a> {
    indices: Peekable<CharIndices<'a>>,
    text: &'a str,
}

impl<'a> Iter<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            indices: text.char_indices().peekable(),
            text,
        }
    }

    pub fn as_full_str(&self) -> &'a str {
        self.text
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Range;

    fn next(&mut self) -> Option<Self::Item> {
        let (maybe_signal_index, maybe_signal_ch) = self.indices.next()?;
        if maybe_signal_ch == SIGNAL_CHAR {
            let Some((first_signal_index, first_signal_ch)) = self.indices.peek().copied() else {
                return Some(Range::empty_signal(maybe_signal_index));
            };
            if first_signal_ch.is_whitespace() {
                return Some(Range::empty_signal(maybe_signal_index));
            } else if let Some(bracket_index) = LEFT_BRACKET_CHARS
                .iter()
                .position(|ch| *ch == first_signal_ch)
            {
                self.indices.next();
                let Some((param_start, _)) = self.indices.next() else {
                    return Some(Range::empty_signal(maybe_signal_index));
                };
                for (param_index, param_ch) in &mut self.indices {
                    if param_ch == RIGHT_BRACKET_CHARS[bracket_index] {
                        return Some(Range::nameless_signal(param_start..param_index));
                    }
                }
                return Some(Range::nameless_signal(param_start..self.text.len()));
            }
            self.indices.next();
            while let Some((name_index, name_ch)) = self.indices.peek().copied() {
                if name_ch.is_whitespace() {
                    return Some(Range::paramless_signal(first_signal_index..name_index));
                } else if let Some(bracket_index) =
                    LEFT_BRACKET_CHARS.iter().position(|ch| *ch == name_ch)
                {
                    self.indices.next();
                    let Some((param_start, _)) = self.indices.next() else {
                        return Some(Range::paramless_signal(first_signal_index..name_index));
                    };
                    for (param_index, param_ch) in &mut self.indices {
                        if param_ch == RIGHT_BRACKET_CHARS[bracket_index] {
                            return Some(Range::Signal {
                                prompt: first_signal_index..name_index,
                                param: param_start..param_index,
                            });
                        }
                    }
                    return Some(Range::Signal {
                        prompt: first_signal_index..name_index,
                        param: param_start..self.text.len(),
                    });
                }
                self.indices.next();
            }
            return Some(Range::paramless_signal(first_signal_index..self.text.len()));
        }
        while let Some((text_index, text_ch)) = self.indices.peek().copied() {
            if text_ch == SIGNAL_CHAR {
                return Some(Range::Text(maybe_signal_index..text_index));
            }
            self.indices.next();
        }
        self.indices.next();
        Some(Range::Text(maybe_signal_index..self.text.len()))
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl<'a> FusedIterator for Iter<'a> {}

#[cfg(test)]
mod tests {
    use super::{Iter, Range};

    #[test]
    fn just_text() {
        const SAMPLE: &str = "Hello, world!";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Text(range0) = &range_event0 else {
            panic!("expected text range, got {range_event0:?}");
        };
        assert_eq!(&SAMPLE[range0.clone()], SAMPLE);
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn empty_signals() {
        const SAMPLE: &str = "Hello, @ world! @";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Text(range0) = &range_event0 else {
            panic!("expected text range, got {range_event0:?}");
        };
        assert_eq!(&SAMPLE[range0.clone()], "Hello, ");
        let range_event1 = iter.next().expect("second range event");
        let Range::Signal {
            prompt: name,
            param,
        } = &range_event1
        else {
            panic!("expected signal range, got {range_event1:?}");
        };
        assert!(name.is_empty());
        assert!(param.is_empty());
        let range_event2 = iter.next().expect("third range event");
        let Range::Text(range2) = &range_event2 else {
            panic!("expected text range, got {range_event2:?}");
        };
        assert_eq!(&SAMPLE[range2.clone()], " world! ");
        let range_event3 = iter.next().expect("fourth range event");
        let Range::Signal {
            prompt: name,
            param,
        } = &range_event3
        else {
            panic!("expected signal range, got {range_event3:?}");
        };
        assert!(name.is_empty());
        assert!(param.is_empty());
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn paramless_signals() {
        const SAMPLE: &str = "@first_signal Hello, @second_signal world!";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Signal {
            prompt: name,
            param,
        } = &range_event0
        else {
            panic!("expected signal range, got {range_event0:?}");
        };
        assert_eq!(&SAMPLE[name.clone()], "first_signal");
        assert!(param.is_empty());
        let range_event1 = iter.next().expect("second range event");
        let Range::Text(range1) = &range_event1 else {
            panic!("expected text range, got {range_event1:?}");
        };
        assert_eq!(&SAMPLE[range1.clone()], " Hello, ");
        let range_event2 = iter.next().expect("third range event");
        let Range::Signal {
            prompt: name,
            param,
        } = &range_event2
        else {
            panic!("expected signal range, got {range_event2:?}");
        };
        assert_eq!(&SAMPLE[name.clone()], "second_signal");
        assert!(param.is_empty());
        let range_event3 = iter.next().expect("fourth range event");
        let Range::Text(range3) = &range_event3 else {
            panic!("expected text range, got {range_event3:?}");
        };
        assert_eq!(&SAMPLE[range3.clone()], " world!");
        assert_eq!(iter.next(), None);
    }

    #[test]
    fn full_signals() {
        const SAMPLE: &str = "Hello, @first_signal{ 20 84 }@second_signal{ #e13f3f } world!";
        let mut iter = Iter::new(SAMPLE);
        let range_event0 = iter.next().expect("first range event");
        let Range::Text(range0) = &range_event0 else {
            panic!("expected text range, got {range_event0:?}");
        };
        assert_eq!(&SAMPLE[range0.clone()], "Hello, ");
        let range_event1 = iter.next().expect("second range event");
        let Range::Signal {
            prompt: name,
            param,
        } = &range_event1
        else {
            panic!("expected signal range, got {range_event1:?}");
        };
        assert_eq!(&SAMPLE[name.clone()], "first_signal");
        assert_eq!(&SAMPLE[param.clone()], " 20 84 ");
        let range_event2 = iter.next().expect("second range event");
        let Range::Signal {
            prompt: name,
            param,
        } = &range_event2
        else {
            panic!("expected signal range, got {range_event2:?}");
        };
        assert_eq!(&SAMPLE[name.clone()], "second_signal");
        assert_eq!(&SAMPLE[param.clone()], " #e13f3f ");
        let range_event3 = iter.next().expect("fourth range event");
        let Range::Text(range3) = &range_event3 else {
            panic!("expected text range, got {range_event3:?}");
        };
        assert_eq!(&SAMPLE[range3.clone()], " world!");
        assert_eq!(iter.next(), None);
    }
}
