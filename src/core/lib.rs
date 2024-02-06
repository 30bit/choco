#![cfg_attr(not(test), no_std)]

mod event;
mod lines;
mod raw;
mod trim;

pub use event::{Event, Iter, Signal, StrRange};
