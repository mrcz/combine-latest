# combine-latest

[![CI](https://github.com/mrcz/combine-latest/actions/workflows/ci.yml/badge.svg)](https://github.com/mrcz/combine-latest/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/combine-latest.svg)](https://crates.io/crates/combine-latest)
[![docs.rs](https://docs.rs/combine-latest/badge.svg)](https://docs.rs/combine-latest)

Combines two or more streams into a new stream which yields tuples with the latest values from each
input stream. Inspired by RxJS's `combineLatest`. The trait-based API supports up to 12 streams;
free functions are available for 2–4.

```rust
use combine_latest::CombineLatest;
use futures::executor::block_on;
use futures::stream::{self, StreamExt};

let temperature = stream::iter([25, 26, 25]);
let weather_notes = stream::iter(["Low visibility", "Foggy"]);
let result: Vec<_> = block_on(
    (temperature, weather_notes).combine_latest().collect()
);
```

`combine_latest` won't yield its first tuple until all input streams have yielded a value. If you
need to get items as soon as the first is available, use `combine_latest_opt` /
`CombineLatestOpt` which yields tuples of `Option`s.

As values come in over time on two input streams, `combine_latest` and `combine_latest_opt` will
yield values like so:

```text
temperature ──┐
              ├── combine_latest ──► (T, W) tuples
weather_notes ┘
```

| time | temperature | weather_notes  | combine_latest         | combine_latest_opt                 |
|------|-------------|----------------|------------------------|------------------------------------|
| 0    | 25          |                |                        | (Some(25), None)                   |
| 1    | 26          |                |                        | (Some(26), None)                   |
| 2    |             | Low visibility | (26, "Low visibility") | (Some(26), Some("Low visibility")) |
| 3    |             | Foggy          | (26, "Foggy")          | (Some(26), Some("Foggy"))          |
| 4    | 25          |                | (25, "Foggy")          | (Some(25), Some("Foggy"))          |

Since the same input value might be returned several times in the output stream, the items that the
input streams yield *must implement Clone.*

For types that don't implement Clone, use `map_latest` / `MapLatest` which passes references to a
closure instead:

```rust
use combine_latest::MapLatest;
use futures::executor::block_on;
use futures::stream::{self, StreamExt};

struct NonClone(i32);

let s1 = stream::iter([NonClone(1), NonClone(2)]);
let s2 = stream::iter([NonClone(10)]);
let result: Vec<_> = block_on(
    (s1, s2).map_latest(|a, b| a.0 + b.0).collect()
);
assert_eq!(result, vec![11, 12]);
```

Using the same input streams with `map_latest(|t, w| format!("{t}°: {w}"))`:

```text
temperature ──┐
              ├── map_latest(|t, w| ...) ──► closure results
weather_notes ┘
```

| time | temperature | weather_notes  | map_latest output     |
|------|-------------|----------------|-----------------------|
| 0    | 25          |                |                       |
| 1    | 26          |                |                       |
| 2    |             | Low visibility | "26°: Low visibility" |
| 3    |             | Foggy          | "26°: Foggy"          |
| 4    | 25          |                | "25°: Foggy"          |

## Free functions

All combinators are also available as free functions for 2–4 streams:

```rust
use combine_latest::{combine_latest, map_latest3};
use futures::executor::block_on;
use futures::stream::{self, StreamExt};

let s1 = stream::iter([1, 2, 3]);
let s2 = stream::iter(["a", "b"]);
let result: Vec<_> = block_on(combine_latest(s1, s2).collect());
assert_eq!(result, vec![(1, "a"), (2, "a"), (2, "b"), (3, "b")]);

let s1 = stream::iter([1, 2]);
let s2 = stream::iter(["a"]);
let s3 = stream::iter([true]);
let result: Vec<_> = block_on(map_latest3(s1, s2, s3, |n, s, b| {
    format!("{n}-{s}-{b}")
}).collect());
assert_eq!(result, vec!["1-a-true", "2-a-true"]);
```

## `with_latest_from`

`with_latest_from` is like `combine_latest`, but only emits when the **primary** (first) stream
yields a value. Secondary streams silently update their cached values in the background. This is
useful when one stream drives the logic and the others provide context — for example, emitting on
each user click while attaching the latest form state.

```text
clicks (primary) ──┐
                   ├── with_latest_from ──► (Click, State) tuples
form_state ────────┘   (emits only when primary yields)
```

```rust
use combine_latest::WithLatestFrom;
use futures::executor::block_on;
use futures::stream::{self, StreamExt};

let clicks = stream::iter([1, 2, 3]);
let form_state = stream::iter(["draft"]);
let result: Vec<_> = block_on(
    (clicks, form_state).with_latest_from().collect()
);
// Click 1 is skipped because form_state hasn't yielded yet
assert_eq!(result, vec![(2, "draft"), (3, "draft")]);
```

`MapWithLatestFrom` is the reference-based variant (no `Clone` required).

## Installation

```sh
cargo add combine-latest
```

## Minimum Supported Rust Version

Rust 1.85 or later (edition 2024).
