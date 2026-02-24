#![forbid(unsafe_code)]

mod stream_combiners;

#[allow(deprecated)]
pub use stream_combiners::combine_latest_optional;
pub use stream_combiners::{
    combine_latest, combine_latest_opt, combine_latest_opt3, combine_latest_opt4, combine_latest3,
    combine_latest4, map_latest, map_latest_opt, map_latest_opt3, map_latest_opt4, map_latest3,
    map_latest4,
};
