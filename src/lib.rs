//! # Syntax
//!
//! Every signal is prefixed with `@`-character. Signals may contain
//! - just a prompt (e.g. `@wave`)
//! - just a parameter (e.g. `@{ My important param }`)
//! - both prompt and parameter (e.g. `@bookmark{into}`)
//! - or neither (e.g. `Pay attention! @`).
//!
//! Three signal prompts are taken by Choco. These are `bookmark`, `choice` and `style`.
//!
//! ### Branching
//!
//! Branching is easy in Choco. `@bookmark{bookmark-name}` registers a graph node, and `@choice{chosen-bookmark-name}` creates an edge between bookmark this choice belongs to and chosen bookmark. For example:
//!
//! ```text
//! @bookmark{greet}
//! – Hello, you!
//! @choice{greet}– Come again?
//! @choice{bye}– Hi!
//!
//! @bookmark{bye}
//! – Well, farewell..
//! ```
//!
//! ### Styling
//!
//! Styling text is done with `@style` signal. It accepts a mix of shortened to one character style names and prefixes promptless parameter, containing text.
//! For example,
//!
//! ```text
//! @style{qbp}@{- Hello, you!}
//! ```
//!
//! Style names are slightly opinionated, but you decide how to display a mix of them:
//!
//! | Char | Style       | Note                           |
//! | ---- | ----------- | ------------------------------ |
//! | p    | Panel       | i.e. block                     |
//! | c    | `Code`      |                                |
//! | q    | > Quote     | doesn't have to be block-quote |
//! | b    | *Bold*      |                                |
//! | i    | **Italic**  |                                |
//! | s    | ~~Scratch~~ | i.e. strike-through            |

mod core;
mod graph;
mod style;

pub use petgraph;

pub use core::{Signal, StrRange};
pub use graph::{read, Guide, Story};
pub use style::{event_iter, Event, EventIter, Style};
