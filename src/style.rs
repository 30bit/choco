use crate::core::{Event as CoreEvent, Iter as CoreIter, Signal, StrRange};
use bitflags::bitflags;
use std::iter::Peekable;

bitflags! {
    #[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Default)]
    pub struct Style: u16 {
        const REGULAR = 0b0000_0000_0000_0000;
        const PANEL = 0b0000_0000_0000_0001;
        const CODE = 0b0000_0000_0000_0010;
        const QUOTE = 0b0000_0000_0000_0100;
        const BOLD =  0b0000_0000_0000_1000;
        const ITALIC = 0b0000_0000_0001_0000;
        const SCRATCH = 0b0000_0000_0010_0000;
    }
}

impl Style {
    fn from_param(param: &str) -> Self {
        let mut style = Style::REGULAR;
        for ch in param.chars() {
            style |= match ch {
                'p' => Style::PANEL,
                'c' => Style::CODE,
                'q' => Style::QUOTE,
                'b' => Style::BOLD,
                'i' => Style::ITALIC,
                's' => Style::SCRATCH,
                _ => Style::REGULAR,
            }
        }
        style
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Event<'a> {
    Signal(Signal<'a>),
    Text { style: Style, content: StrRange<'a> },
    Break,
}

impl<'a> Event<'a> {
    fn from_inner(event: CoreEvent<'a>) -> Self {
        match event {
            CoreEvent::Signal(sig) => Self::Signal(sig),
            CoreEvent::Text(content) => Self::Text {
                style: Style::REGULAR,
                content,
            },
            CoreEvent::Break => Self::Break,
        }
    }
}

#[derive(Clone, Debug)]
pub struct EventIter<'a> {
    inner: Peekable<CoreIter<'a>>,
}

impl<'a> EventIter<'a> {
    #[must_use]
    pub fn new(text: &'a str) -> Self {
        Self {
            inner: CoreIter::new(text).peekable(),
        }
    }
}

/// Go through text and parse signals out
#[must_use]
pub fn event_iter(text: &str) -> EventIter {
    EventIter::new(text)
}

fn event_to_param<'a>(event: &CoreEvent<'a>) -> Option<StrRange<'a>> {
    match event {
        CoreEvent::Signal(Signal::Param(param)) => Some(param.clone()),
        _ => None,
    }
}

fn event_to_style(event: &CoreEvent) -> Option<Style> {
    match &event {
        CoreEvent::Signal(Signal::Call {
            prompt: StrRange { slice: "style", .. },
            param,
        }) => Some(Style::from_param(param.slice)),
        _ => None,
    }
}

impl<'a> Iterator for EventIter<'a> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.inner.next()?;

        if let Some(style) = event_to_style(&next) {
            let peek = self.inner.peek()?;
            let param = event_to_param(peek)?;
            self.inner.next();
            Some(Event::Text {
                style,
                content: param,
            })
        } else {
            Some(Event::from_inner(next))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, EventIter, Style};

    #[test]
    fn style() {
        const SAMPLE: &str = "@style{bcqi}@{Hello}, world!";
        let mut iter = EventIter::new(SAMPLE);
        let next = iter.next().unwrap();
        let Event::Text { style, content } = next else {
            panic!("expected text");
        };
        assert_eq!(
            style,
            Style::BOLD | Style::CODE | Style::QUOTE | Style::ITALIC
        );
        assert_eq!(content.slice, "Hello");
        let next = iter.next().unwrap();
        let Event::Text { style, content } = next else {
            panic!("expected text");
        };
        assert_eq!(style, Style::REGULAR);
        assert_eq!(content.slice, ", world!");
    }
}
