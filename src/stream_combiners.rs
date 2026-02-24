use std::future;

use futures_core::Stream;
use futures_util::{StreamExt, stream::select};

enum LR<L, R> {
    L(L),
    R(R),
}

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
    select(s1.map(LR::L), s2.map(LR::R)).map(move |t1_or_t2| {
        match t1_or_t2 {
            LR::L(t1) => current1 = Some(t1),
            LR::R(t2) => current2 = Some(t2),
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

    select(s1.map(LR::L), s2.map(LR::R)).filter_map(move |t1_or_t2| {
        match t1_or_t2 {
            LR::L(t1) => current1 = Some(t1),
            LR::R(t2) => current2 = Some(t2),
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
    mut f: impl for<'a, 'b> FnMut(Option<&'a T1>, Option<&'b T2>) -> U,
) -> impl Stream<Item = U> {
    let mut current1 = None;
    let mut current2 = None;

    select(s1.map(LR::L), s2.map(LR::R)).map(move |t1_or_t2| {
        match t1_or_t2 {
            LR::L(t1) => current1 = Some(t1),
            LR::R(t2) => current2 = Some(t2),
        };
        f(current1.as_ref(), current2.as_ref())
    })
}

// -- 3-stream combinators --

enum Tag3<A, B, C> {
    A(A),
    B(B),
    C(C),
}

/// Combines three streams into a new stream that yields the latest items from all streams as a
/// tuple. This stream won't yield until all three input streams have yielded at least one item
/// each.
pub fn combine_latest3<T1: Clone, T2: Clone, T3: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
) -> impl Stream<Item = (T1, T2, T3)> {
    combine_latest_opt3(s1, s2, s3)
        .filter_map(|(v1, v2, v3)| future::ready(v1.zip(v2).zip(v3).map(|((a, b), c)| (a, b, c))))
}

/// Combines three streams into a new stream, yielding tuples of
/// `(Option<T1>, Option<T2>, Option<T3>)`. The stream starts yielding as soon as any input stream
/// yields an item.
pub fn combine_latest_opt3<T1: Clone, T2: Clone, T3: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
) -> impl Stream<Item = (Option<T1>, Option<T2>, Option<T3>)> {
    let mut c1 = None;
    let mut c2 = None;
    let mut c3 = None;
    select(select(s1.map(Tag3::A), s2.map(Tag3::B)), s3.map(Tag3::C)).map(move |tagged| {
        match tagged {
            Tag3::A(v) => c1 = Some(v),
            Tag3::B(v) => c2 = Some(v),
            Tag3::C(v) => c3 = Some(v),
        };
        (c1.clone(), c2.clone(), c3.clone())
    })
}

/// Combines three streams and applies the given function to each set of latest items. The function
/// takes references, so the types don't need to implement `Clone`. Won't yield until all three
/// streams have yielded at least one item each.
pub fn map_latest3<T1, T2, T3, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    mut f: impl for<'a, 'b, 'c> FnMut(&'a T1, &'b T2, &'c T3) -> U,
) -> impl Stream<Item = U> {
    let mut c1 = None;
    let mut c2 = None;
    let mut c3 = None;

    select(select(s1.map(Tag3::A), s2.map(Tag3::B)), s3.map(Tag3::C)).filter_map(move |tagged| {
        match tagged {
            Tag3::A(v) => c1 = Some(v),
            Tag3::B(v) => c2 = Some(v),
            Tag3::C(v) => c3 = Some(v),
        };
        future::ready(
            c1.as_ref()
                .zip(c2.as_ref())
                .zip(c3.as_ref())
                .map(|((a, b), c)| f(a, b, c)),
        )
    })
}

/// Combines three streams and applies the given function to each set of latest items. The function
/// takes `Option` references and yields as soon as any input stream yields.
pub fn map_latest_opt3<T1, T2, T3, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    mut f: impl for<'a, 'b, 'c> FnMut(Option<&'a T1>, Option<&'b T2>, Option<&'c T3>) -> U,
) -> impl Stream<Item = U> {
    let mut c1 = None;
    let mut c2 = None;
    let mut c3 = None;

    select(select(s1.map(Tag3::A), s2.map(Tag3::B)), s3.map(Tag3::C)).map(move |tagged| {
        match tagged {
            Tag3::A(v) => c1 = Some(v),
            Tag3::B(v) => c2 = Some(v),
            Tag3::C(v) => c3 = Some(v),
        };
        f(c1.as_ref(), c2.as_ref(), c3.as_ref())
    })
}

// -- 4-stream combinators --

enum Tag4<A, B, C, D> {
    A(A),
    B(B),
    C(C),
    D(D),
}

/// Combines four streams into a new stream that yields the latest items from all streams as a
/// tuple. This stream won't yield until all four input streams have yielded at least one item each.
pub fn combine_latest4<T1: Clone, T2: Clone, T3: Clone, T4: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
) -> impl Stream<Item = (T1, T2, T3, T4)> {
    combine_latest_opt4(s1, s2, s3, s4).filter_map(|(v1, v2, v3, v4)| {
        future::ready(
            v1.zip(v2)
                .zip(v3)
                .zip(v4)
                .map(|(((a, b), c), d)| (a, b, c, d)),
        )
    })
}

/// Combines four streams into a new stream, yielding tuples of
/// `(Option<T1>, Option<T2>, Option<T3>, Option<T4>)`. The stream starts yielding as soon as any
/// input stream yields an item.
pub fn combine_latest_opt4<T1: Clone, T2: Clone, T3: Clone, T4: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
) -> impl Stream<Item = (Option<T1>, Option<T2>, Option<T3>, Option<T4>)> {
    let mut c1 = None;
    let mut c2 = None;
    let mut c3 = None;
    let mut c4 = None;
    select(
        select(select(s1.map(Tag4::A), s2.map(Tag4::B)), s3.map(Tag4::C)),
        s4.map(Tag4::D),
    )
    .map(move |tagged| {
        match tagged {
            Tag4::A(v) => c1 = Some(v),
            Tag4::B(v) => c2 = Some(v),
            Tag4::C(v) => c3 = Some(v),
            Tag4::D(v) => c4 = Some(v),
        };
        (c1.clone(), c2.clone(), c3.clone(), c4.clone())
    })
}

/// Combines four streams and applies the given function to each set of latest items. The function
/// takes references, so the types don't need to implement `Clone`. Won't yield until all four
/// streams have yielded at least one item each.
pub fn map_latest4<T1, T2, T3, T4, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
    mut f: impl for<'a, 'b, 'c, 'd> FnMut(&'a T1, &'b T2, &'c T3, &'d T4) -> U,
) -> impl Stream<Item = U> {
    let mut c1 = None;
    let mut c2 = None;
    let mut c3 = None;
    let mut c4 = None;

    select(
        select(select(s1.map(Tag4::A), s2.map(Tag4::B)), s3.map(Tag4::C)),
        s4.map(Tag4::D),
    )
    .filter_map(move |tagged| {
        match tagged {
            Tag4::A(v) => c1 = Some(v),
            Tag4::B(v) => c2 = Some(v),
            Tag4::C(v) => c3 = Some(v),
            Tag4::D(v) => c4 = Some(v),
        };
        future::ready(
            c1.as_ref()
                .zip(c2.as_ref())
                .zip(c3.as_ref())
                .zip(c4.as_ref())
                .map(|(((a, b), c), d)| f(a, b, c, d)),
        )
    })
}

/// Combines four streams and applies the given function to each set of latest items. The function
/// takes `Option` references and yields as soon as any input stream yields.
pub fn map_latest_opt4<T1, T2, T3, T4, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
    mut f: impl for<'a, 'b, 'c, 'd> FnMut(
        Option<&'a T1>,
        Option<&'b T2>,
        Option<&'c T3>,
        Option<&'d T4>,
    ) -> U,
) -> impl Stream<Item = U> {
    let mut c1 = None;
    let mut c2 = None;
    let mut c3 = None;
    let mut c4 = None;

    select(
        select(select(s1.map(Tag4::A), s2.map(Tag4::B)), s3.map(Tag4::C)),
        s4.map(Tag4::D),
    )
    .map(move |tagged| {
        match tagged {
            Tag4::A(v) => c1 = Some(v),
            Tag4::B(v) => c2 = Some(v),
            Tag4::C(v) => c3 = Some(v),
            Tag4::D(v) => c4 = Some(v),
        };
        f(c1.as_ref(), c2.as_ref(), c3.as_ref(), c4.as_ref())
    })
}
