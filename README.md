# combine-latest

[![CI](https://github.com/mrcz/combine-latest/actions/workflows/ci.yml/badge.svg)](https://github.com/mrcz/combine-latest/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/combine-latest.svg)](https://crates.io/crates/combine-latest)
[![docs.rs](https://docs.rs/combine-latest/badge.svg)](https://docs.rs/combine-latest)

Combines two or more streams into a new stream which yields tuples with the latest values from each
input stream. Inspired by RxJS's `combineLatest`. Supports 2, 3, and 4 input streams.

```rust
use async_stream::stream;
use combine_latest::combine_latest;
use futures_core::Stream;

fn combine_weather_data_streams(
    temperature: impl Stream<Item = i32>,
    weather_notes: impl Stream<Item = String>,
) -> impl Stream<Item = String> {
    stream! {
        for await (t, n) in combine_latest(temperature, weather_notes) {
            yield format!("Temperature {t}°, note: {n}");
        }
    }
}
```

`combine_latest` won't yield its first tuple until both input streams have yielded a value. If you
need to get items as soon as the first is available, there is a `combine_latest_opt` function that
yields `(Option<T1>, Option<T2>)` tuples.

As values come in over time on the `temperature` and `weather_notes` streams, `combine_latest` and
`combine_latest_opt` will yield values like so:

| time | temperature | weather_notes  | combine_latest         | combine_latest_opt                 |
|------|-------------|----------------|------------------------|------------------------------------|
| 0    | 25          |                |                        | (Some(25), None)                   |
| 1    | 26          |                |                        | (Some(26), None)                   |
| 2    |             | Low visibility | (26, "Low visibility") | (Some(26), Some("Low visibility")) |
| 3    |             | Foggy          | (26, "Foggy")          | (Some(26), Some("Foggy"))          |
| 4    | 25          |                | (25, "Foggy")          | (Some(25), Some("Foggy"))          |

Since the same input value might be returned several times in the output stream, the items that the
input streams yield *must implement Clone.*

For types that don't implement Clone, it's possible to use the function `map_latest`:

```rust
use async_stream::stream;
use combine_latest::map_latest;
use futures_core::Stream;

struct NonCloneTemperature(i32);

fn combine_weather_data_streams(
    temperature: impl Stream<Item = NonCloneTemperature>,
    weather_notes: impl Stream<Item = String>,
) -> impl Stream<Item = String> {
    stream! {
        for await output in map_latest(
            temperature,
            weather_notes,
            |t, n| format!("Temperature {}°, note: {n}", t.0),
        ) {
            yield output;
        }
    }
}
```

## Combining 3 or 4 streams

All functions have `3` and `4` variants for combining more streams:

```rust
use combine_latest::{combine_latest3, map_latest4};
use futures::executor::block_on;
use futures::stream::{self, StreamExt};

// Combine 3 streams into tuples
let s1 = stream::iter([1, 2]);
let s2 = stream::iter(["a", "b"]);
let s3 = stream::iter([true]);
let result: Vec<_> = block_on(combine_latest3(s1, s2, s3).collect());

// Or use map_latest4 to apply a function over 4 streams
let s1 = stream::iter([1]);
let s2 = stream::iter(["a"]);
let s3 = stream::iter([true]);
let s4 = stream::iter([0.5_f64]);
let result: Vec<_> = block_on(map_latest4(s1, s2, s3, s4, |a, b, c, d| {
    format!("{a}-{b}-{c}-{d}")
}).collect());
```

## Installation

```sh
cargo add combine-latest
```

## Minimum Supported Rust Version

Rust 1.85 or later (edition 2024).
