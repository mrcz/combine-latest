#![forbid(unsafe_code)]

mod stream_combiners;

#[allow(deprecated)]
pub use stream_combiners::combine_latest_optional;
pub use stream_combiners::{combine_latest, combine_latest_opt, map_latest, map_latest_opt};
