#![forbid(unsafe_code)]

mod stream_combiners;
pub use stream_combiners::{
    combine_latest, combine_latest_opt, combine_latest_optional, map_latest, map_latest_opt,
};
