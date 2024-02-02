[![choco crate](https://img.shields.io/crates/v/choco.svg)](https://crates.io/crates/choco)
[![choco documentation](https://docs.rs/choco/badge.svg)](https://docs.rs/choco)
[![workflow run status](https://github.com/30bit/choco/actions/workflows/ci.yml/badge.svg)](https://github.com/30bit/choco/actions/workflows/ci.yml)

Choco is a markup language for dialogue systems. It works by emitting signals from text into rust via special `@`-syntax.

# Syntax

Every signal is prefixed with `@`-character. Signals may contain 
- just a prompt (e.g. `@wave`)
- just a parameter (e.g. `@{ My important param }`) 
- both prompt and parameter (e.g. `@bookmark{into}`) 
- or neither (e.g. `Pay attention! @`).

Three signal prompts are taken by Choco. These are `bookmark`, `choice` and `style`.

### Branching

Branching is easy in Choco. `@bookmark{bookmark-name}` registers a graph node, and `@choice{chosen-bookmark-name}` creates an edge between bookmark this choice belongs to and chosen bookmark. For example:

```
@bookmark{greet}
– Hello, you!
@choice{greet}– Come again?
@choice{bye}– Hi!

@bookmark{bye}
– Well, farewell..
```

### Styling

Styling text is done with `@style` signal. It accepts a mix of shortened to one character style names and prefixes promptless parameter, containing text.
For example,

```
@style{qbp}@{- Hello, you!}
```

Style names are slightly opinionated, but you decide how to display a mix of them:

| Char | Style       | Note                           |
| ---- | ----------- | ------------------------------ |
| p    | Panel       | i.e. block                     |
| c    | `Code`      |                                |
| q    | > Quote     | doesn't have to be block-quote |
| b    | *Bold*      |                                |
| i    | **Italic**  |                                |
| s    | ~~Scratch~~ | i.e. strike-through            |

# License

Licensed under either of [Apache License, Version 2.0](LICENSE-APACHE) or [MIT license](LICENSE-MIT) at your option.

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in this crate by you, as defined in the Apache-2.0 license, shall
be dual licensed as above, without any additional terms or conditions.