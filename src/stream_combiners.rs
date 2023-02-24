use std::future;

use either::Either;
use futures_core::Stream;
use futures_util::{stream::select, StreamExt};

/// Combines two streams into a new stream that always contains the latest items from both streams
/// as a tuple. This stream won't yield a tuple until both input streams have yielded at least one
/// item each.
pub fn combine_latest<T1: Clone, T2: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
) -> impl Stream<Item = (T1, T2)> {
    combine_latest_opt(s1, s2).filter_map(|(v1, v2)| future::ready(v1.zip(v2)))
}

#[deprecated(since = "1.1.0", note = "Use combine_latest_opt instead")]
pub fn combine_latest_optional<T1: Clone, T2: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
) -> impl Stream<Item = (Option<T1>, Option<T2>)> {
    combine_latest_opt(s1, s2)
}

/// Combines two streams into a new stream, yielding tuples of `(Option<T1>, Option<T2>)`. The
/// stream starts yielding tuples as soon as one of the input streams yields an item, and the one
/// that has not yet yielded has a corresponding `None` in its field of the tuple.
pub fn combine_latest_opt<T1: Clone, T2: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
) -> impl Stream<Item = (Option<T1>, Option<T2>)> {
    let mut current1 = None;
    let mut current2 = None;
    select(s1.map(Either::Left), s2.map(Either::Right)).map(move |tuple| {
        match tuple {
            Either::Left(i1) => current1 = Some(i1),
            Either::Right(i2) => current2 = Some(i2),
        };
        (current1.clone(), current2.clone())
    })
}

/// Combines two streams into a new stream and apply the given function to each item. The function
/// takes references as arguments, so unlike `combine_latest` the types T1 and T2 don't have to
/// implement `Clone`. The returned stream won't yield until both streams have yielded at least one
/// item each.
pub fn map_latest<T1, T2, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    mut f: impl for<'a, 'b> FnMut(&'a T1, &'b T2) -> U,
) -> impl Stream<Item = U> {
    let mut current1 = None;
    let mut current2 = None;

    select(s1.map(Either::Left), s2.map(Either::Right)).filter_map(move |tuple| {
        match tuple {
            Either::Left(i1) => current1 = Some(i1),
            Either::Right(i2) => current2 = Some(i2),
        };
        future::ready(
            current1
                .as_ref()
                .zip(current2.as_ref())
                .map(|args| f(args.0, args.1)),
        )
    })
}

/// Combines two streams into a new stream and apply the given function to each item. The function
/// takes references as arguments, so unlike `combine_latest` the types T1 and T2 don't have to
/// implement `Clone`. The returned stream will yield as soon as one input stream yields.
pub fn map_latest_opt<T1, T2, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    mut f: impl for<'a, 'b> FnMut(&'a Option<T1>, &'b Option<T2>) -> U,
) -> impl Stream<Item = U> {
    let mut current1 = None;
    let mut current2 = None;

    select(s1.map(Either::Left), s2.map(Either::Right)).map(move |tuple| {
        match tuple {
            Either::Left(i1) => current1 = Some(i1),
            Either::Right(i2) => current2 = Some(i2),
        };
        f(&current1, &current2)
    })
}
