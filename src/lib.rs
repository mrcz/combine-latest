//! Combine two or more async streams, emitting tuples of the latest values from each input.
//!
//! ```
//! use combine_latest::CombineLatest;
//! use futures::executor::block_on;
//! use futures::stream::{self, StreamExt};
//!
//! let s1 = stream::iter([1, 2, 3]);
//! let s2 = stream::iter(["a", "b"]);
//! let result: Vec<_> = block_on((s1, s2).combine_latest().collect());
//! assert_eq!(result, vec![(1, "a"), (2, "a"), (2, "b"), (3, "b")]);
//! ```
//!
//! The trait-based API ([`CombineLatest`], [`CombineLatestOpt`], [`MapLatest`], [`MapLatestOpt`],
//! [`WithLatestFrom`], [`MapWithLatestFrom`]) supports tuples of 2 to 12 streams. Free functions
//! ([`combine_latest`], [`map_latest`], etc.) are available for 2 to 4 streams.

#![forbid(unsafe_code)]

mod stream_combiners;

#[allow(deprecated)]
pub use stream_combiners::combine_latest_optional;
pub use stream_combiners::{
    CombineLatest, CombineLatestOpt, MapLatest, MapLatestOpt, MapWithLatestFrom, WithLatestFrom,
    combine_latest, combine_latest_opt, combine_latest_opt3, combine_latest_opt4, combine_latest3,
    combine_latest4, map_latest, map_latest_opt, map_latest_opt3, map_latest_opt4, map_latest3,
    map_latest4, map_with_latest_from, map_with_latest_from3, map_with_latest_from4,
    with_latest_from, with_latest_from3, with_latest_from4,
};
