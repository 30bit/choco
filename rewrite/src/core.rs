use ::core::{
    any::{Any, TypeId},
    iter::{FusedIterator, Peekable},
    ops::Range,
    str::CharIndices,
};
use core::fmt;

const SIGNAL_CHAR: char = '@';

#[derive(Clone, Eq, PartialEq, Hash, Debug, Default)]
pub struct StrRange<'a> {
    /// original [`str`]
    pub full: &'a str,
    /// byte-index range in original text
    pub range: Range<usize>,
}

impl<'a> StrRange<'a> {
    /// Slices full [`str`] by [`Self::range`]
    ///
    /// # Panics
    ///
    /// If [`Self::range`] is out of passed [`str`] bounds
    #[inline]
    #[must_use]
    pub fn substr(&self) -> &'a str {
        &self.full[self.range.clone()]
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct PluginError {
    pub plugin: TypeId,
    pub signal_range: Range<usize>,
    pub msg: &'static str,
}

impl PluginError {
    #[inline]
    #[must_use]
    pub fn new<P: Plugin>(signal: Range<usize>) -> Self {
        Self {
            plugin: TypeId::of::<P>(),
            signal_range: signal,
            msg: "",
        }
    }

    #[inline]
    #[must_use]
    pub fn with_msg(mut self, msg: &'static str) -> Self {
        self.msg = msg;
        self
    }
}

impl fmt::Display for PluginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "signal `[{:?}]` can't be taken by plugin: {}",
            self.signal_range, self.msg
        )
    }
}

pub type PluginResult<T> = Result<T, PluginError>;

pub trait Plugin: Any + Sized {
    /// Try to process `signal`.
    /// If [`Plugin`] implementation doesn't work with that `signal` return [`None`].
    /// Otherwise return [`TypeId`] of the sub [`Plugin`] that successfully handled the `signal`
    ///
    /// [`EventFlow::plugins`] of the passed `flow` parameter is supposed to contain this implementation as sub plugin.
    fn take_signal<P: Plugin>(signal: StrRange, flow: EventFlow<P>)
        -> PluginResult<Option<TypeId>>;

    /// Mutably get sub plugin. The plugin itself is considered sub plugin
    #[inline]
    #[must_use]
    fn get_sub_mut<P: Plugin>(&mut self) -> Option<&mut P> {
        if TypeId::of::<Self>() == TypeId::of::<P>() {
            let any = self as &mut dyn Any;
            any.downcast_mut()
        } else {
            None
        }
    }
}

impl<T: Plugin, U: Plugin> Plugin for (T, U) {
    fn take_signal<P: Plugin>(
        signal: StrRange,
        mut flow: EventFlow<P>,
    ) -> PluginResult<Option<TypeId>> {
        T::take_signal(signal.clone(), flow.clone())
            .transpose()
            .or_else(|| U::take_signal(signal, flow).transpose())
            .transpose()
    }

    fn get_sub_mut<P: Plugin>(&mut self) -> Option<&mut P> {
        self.0.get_sub_mut().or_else(|| self.1.get_sub_mut())
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum RawEventKind {
    Text,
    Signal,
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct RawEvent {
    /// Byte range of the original full [`str`]
    pub range: Range<usize>,
    pub kind: RawEventKind,
}

impl RawEvent {
    #[inline]
    #[must_use]
    pub fn text(range: Range<usize>) -> Self {
        Self {
            range,
            kind: RawEventKind::Text,
        }
    }

    #[inline]
    #[must_use]
    pub fn signal(range: Range<usize>) -> Self {
        Self {
            range,
            kind: RawEventKind::Signal,
        }
    }

    #[inline]
    pub fn is_text(&self) -> bool {
        matches!(self.kind, RawEventKind::Text)
    }

    #[inline]
    pub fn is_signal(&self) -> bool {
        matches!(self.kind, RawEventKind::Signal)
    }

    /// Bundles [`Self::range`] together with original full [`str`]
    #[inline]
    pub fn as_of<'a>(&self, full: &'a str) -> StrRange<'a> {
        StrRange {
            full,
            range: self.range.clone(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub struct TakenSignal {
    pub range: Range<usize>,
    pub plugin: TypeId,
}

impl TakenSignal {
    #[inline]
    #[must_use]
    pub fn new<P: Plugin>(range: Range<usize>) -> Self {
        Self {
            range,
            plugin: TypeId::of::<P>(),
        }
    }

    /// Bundles [`Self::range`] together with original full [`str`]
    #[inline]
    pub fn as_of<'a>(&self, full: &'a str) -> StrRange<'a> {
        StrRange {
            full,
            range: self.range.clone(),
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash, Debug)]
pub enum Event {
    Raw(RawEvent),
    TakenByPlugin(PluginResult<TakenSignal>),
}

#[derive(Debug)]
pub struct EventFlow<'a, 's, P: Plugin> {
    pub plugins: &'s mut P,
    raw_iter: &'s mut RawEventIter<'a>,
}

impl<'a, 's, P: Plugin> EventFlow<'a, 's, P> {
    #[inline]
    #[must_use]
    pub fn new(plugins: &'s mut P, raw_iter: &'s mut RawEventIter<'a>) -> Self {
        Self { plugins, raw_iter }
    }

    #[inline]
    pub fn swap_plugins<'w, T: Plugin>(
        self,
        new_plugins: &'w mut T,
    ) -> (EventFlow<'a, 'w, T>, &'s mut P)
    where
        's: 'w,
    {
        (
            EventFlow {
                plugins: new_plugins,
                raw_iter: self.raw_iter,
            },
            self.plugins,
        )
    }

    #[inline]
    pub fn full_str(&self) -> &'a str {
        self.raw_iter.full
    }

    /// Obtains an owned [`EventFlow`] with shorter lifetimes given a ref
    pub fn clone<'w>(&'w mut self) -> EventFlow<'a, 'w, P> {
        EventFlow {
            plugins: self.plugins,
            raw_iter: self.raw_iter,
        }
    }
}

impl<'a, 's, P: Plugin> Iterator for EventFlow<'a, 's, P> {
    type Item = Event;

    fn next(&mut self) -> Option<Self::Item> {
        let raw_event = self.raw_iter.next()?;
        Some(match raw_event.kind {
            RawEventKind::Text => Event::Raw(raw_event),
            RawEventKind::Signal => P::take_signal(
                StrRange {
                    full: self.raw_iter.full,
                    range: raw_event.range.clone(),
                },
                self.clone(),
            )
            .transpose()
            .map(|result| {
                Event::TakenByPlugin(result.map(|plugin| TakenSignal {
                    range: raw_event.range.clone(),
                    plugin,
                }))
            })
            .unwrap_or_else(|| Event::Raw(raw_event)),
        })
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.raw_iter.size_hint()
    }
}

impl<'a, 's, P: Plugin> FusedIterator for EventFlow<'a, 's, P> {}

#[derive(Clone, Debug)]
pub struct RawEventIter<'a> {
    indices: Peekable<CharIndices<'a>>,
    full: &'a str,
}

impl<'a> RawEventIter<'a> {
    pub fn new(full: &'a str) -> Self {
        Self {
            indices: full.char_indices().peekable(),
            full,
        }
    }
}

impl<'a> Iterator for RawEventIter<'a> {
    type Item = RawEvent;

    fn next(&mut self) -> Option<Self::Item> {
        let (maybe_signal_index, maybe_signal_ch) = self.indices.next()?;
        if maybe_signal_ch == SIGNAL_CHAR {
            let Some((first_signal_index, first_signal_ch)) = self.indices.peek().copied() else {
                return Some(RawEvent::signal(self.full.len()..self.full.len()));
            };
            if first_signal_ch.is_whitespace() || first_signal_ch == SIGNAL_CHAR {
                return Some(RawEvent::signal(first_signal_index..first_signal_index));
            }
            self.indices.next();
            while let Some((maybe_signal_index, maybe_signal_ch)) = self.indices.peek().copied() {
                if maybe_signal_ch.is_whitespace() || maybe_signal_ch == SIGNAL_CHAR {
                    return Some(RawEvent::signal(first_signal_index..maybe_signal_index));
                }
                self.indices.next();
            }
            Some(RawEvent::signal(first_signal_index..self.full.len()))
        } else {
            let first_text_index = maybe_signal_index;
            while let Some((maybe_signal_index, maybe_signal_ch)) = self.indices.peek().copied() {
                if maybe_signal_ch == SIGNAL_CHAR {
                    return Some(RawEvent::text(first_text_index..maybe_signal_index));
                }
                self.indices.next();
            }
            Some(RawEvent::text(first_text_index..self.full.len()))
        }
    }

    #[inline]
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.indices.size_hint()
    }
}

impl<'a> FusedIterator for RawEventIter<'a> {}
