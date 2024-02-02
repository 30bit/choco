use super::{lines, raw::Range, trim};
use core::ops;

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct StrRange<'a> {
    /// original text sliced by `self.range`
    pub slice: &'a str,
    /// byte-index range in original text
    pub range: ops::Range<usize>,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub enum Signal<'a> {
    #[default]
    /// Just an `@`-char
    Ping,
    /// `@`-char suffixed with name
    Prompt(StrRange<'a>),
    /// `@`-char suffixed braces
    Param(StrRange<'a>),
    /// `@`-char suffixed with name and then braces
    Call {
        prompt: StrRange<'a>,
        param: StrRange<'a>,
    },
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Event<'a> {
    Signal(Signal<'a>),
    Text(StrRange<'a>),
    Break,
}

#[derive(Clone, Debug)]
struct Offset(usize);

impl Offset {
    fn offset_range(&self, range: ops::Range<usize>) -> ops::Range<usize> {
        range.start + self.0..range.end + self.0
    }

    fn slice<'a>(&self, full: &'a str, range: ops::Range<usize>) -> StrRange<'a> {
        StrRange {
            slice: &full[range.clone()],
            range: self.offset_range(range),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Iter<'a> {
    current: Option<trim::Iter<'a>>,
    remainder: lines::Iter<'a>,
    offset: Offset,
}

impl<'a> Iter<'a> {
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self {
            current: None,
            remainder: lines::Iter::new(text),
            offset: Offset(0),
        }
    }
}

impl<'a> Iterator for Iter<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(current) = &mut self.current {
            if let Some(range) = current.next() {
                return Some(match range {
                    Range::Text(range) => {
                        Event::Text(self.offset.slice(current.as_full_str(), range))
                    }
                    Range::Signal { prompt, param } if param.is_empty() && prompt.is_empty() => {
                        Event::Signal(Signal::Ping)
                    }
                    Range::Signal { prompt, param } if prompt.is_empty() => Event::Signal(
                        Signal::Param(self.offset.slice(current.as_full_str(), param)),
                    ),
                    Range::Signal { prompt, param } if param.is_empty() => Event::Signal(
                        Signal::Prompt(self.offset.slice(current.as_full_str(), prompt)),
                    ),
                    Range::Signal { prompt, param } => Event::Signal(Signal::Call {
                        prompt: self.offset.slice(current.as_full_str(), prompt),
                        param: self.offset.slice(current.as_full_str(), param),
                    }),
                });
            }
            self.offset.0 = self.remainder.offset();
            self.current = self.remainder.next();
            return if self.current.is_some() {
                Some(Event::Break)
            } else {
                None
            };
        }
        self.offset.0 = self.remainder.offset();
        self.current = self.remainder.next();
        if self.current.is_some() {
            self.next()
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Iter, Signal, StrRange};

    #[test]
    fn full() {
        const SAMPLE: &str = "- Hello! @wave\n@c{1}@{i<4}- Hi!\n@c{2}@{s>7}- Howdy!@\n";
        let mut iter = Iter::new(SAMPLE);
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Text(StrRange {
                    slice: "- Hello!",
                    ..
                })
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Signal(Signal::Prompt(StrRange { slice: "wave", .. }))
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(matches!(event, Event::Break), "{event:?}");
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Signal(Signal::Call {
                    prompt: StrRange { slice: "c", .. },
                    param: StrRange { slice: "1", .. },
                })
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Signal(Signal::Param(StrRange { slice: "i<4", .. }))
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(
            matches!(event, Event::Text(StrRange { slice: "- Hi!", .. })),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(matches!(event, Event::Break), "{event:?}");
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Signal(Signal::Call {
                    prompt: StrRange { slice: "c", .. },
                    param: StrRange { slice: "2", .. },
                })
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Signal(Signal::Param(StrRange { slice: "s>7", .. }))
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(
            matches!(
                event,
                Event::Text(StrRange {
                    slice: "- Howdy!",
                    ..
                })
            ),
            "{event:?}"
        );
        let event = iter.next().unwrap();
        assert!(matches!(event, Event::Signal(Signal::Ping)), "{event:?}");
        let event = iter.next().unwrap();
        assert!(matches!(event, Event::Break), "{event:?}");
        assert_eq!(iter.next(), None);
    }
}
