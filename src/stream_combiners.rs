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
///
/// # Example
///
/// ```
/// use combine_latest::combine_latest;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1, 2, 3]);
/// let s2 = stream::iter(["a", "b"]);
/// let result: Vec<_> = block_on(combine_latest(s1, s2).collect());
/// assert_eq!(result, vec![(1, "a"), (2, "a"), (2, "b"), (3, "b")]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::combine_latest_opt;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let result: Vec<_> = block_on(combine_latest_opt(s1, s2).collect());
/// assert_eq!(
///     result,
///     vec![
///         (Some(1), None),
///         (Some(1), Some("a")),
///         (Some(2), Some("a")),
///     ]
/// );
/// ```
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

/// Combines two streams into a new stream and applies the given function to each item. The
/// function takes references as arguments, so unlike [`combine_latest`] the types don't have to
/// implement `Clone`. The returned stream won't yield until both streams have yielded at least one
/// item each.
///
/// # Example
///
/// ```
/// use combine_latest::map_latest;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1, 2]);
/// let s2 = stream::iter(["a", "b"]);
/// let result: Vec<_> = block_on(map_latest(s1, s2, |n, s| format!("{n}{s}")).collect());
/// assert_eq!(result, vec!["1a", "2a", "2b"]);
/// ```
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

/// Combines two streams into a new stream and applies the given function to each item. The
/// function takes `Option` references and yields as soon as one input stream yields.
///
/// # Example
///
/// ```
/// use combine_latest::map_latest_opt;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let result: Vec<_> = block_on(map_latest_opt(s1, s2, |n, s| {
///     format!("{n:?}-{s:?}")
/// }).collect());
/// assert_eq!(result, vec!["Some(1)-None", "Some(1)-Some(\"a\")", "Some(2)-Some(\"a\")"]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::combine_latest3;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let result: Vec<_> = block_on(combine_latest3(s1, s2, s3).collect());
/// assert_eq!(result, vec![(1, "a", true), (2, "a", true)]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::combine_latest_opt3;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter::<[bool; 0]>([]);
/// let result: Vec<_> = block_on(combine_latest_opt3(s1, s2, s3).collect());
/// assert_eq!(result, vec![(Some(1), None, None), (Some(1), Some("a"), None)]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::map_latest3;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let result: Vec<_> = block_on(map_latest3(s1, s2, s3, |n, s, b| {
///     format!("{n}-{s}-{b}")
/// }).collect());
/// assert_eq!(result, vec!["1-a-true", "2-a-true"]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::map_latest_opt3;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let result: Vec<_> = block_on(map_latest_opt3(s1, s2, s3, |n, s, b| {
///     (n.copied(), s.copied(), b.copied())
/// }).collect());
/// assert_eq!(result[0], (Some(1), None, None));
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::combine_latest4;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let s4 = stream::iter([0.5]);
/// let result: Vec<_> = block_on(combine_latest4(s1, s2, s3, s4).collect());
/// assert_eq!(result, vec![(1, "a", true, 0.5)]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::combine_latest_opt4;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1]);
/// let s2 = stream::iter::<[&str; 0]>([]);
/// let s3 = stream::iter::<[bool; 0]>([]);
/// let s4 = stream::iter::<[f64; 0]>([]);
/// let result: Vec<_> = block_on(combine_latest_opt4(s1, s2, s3, s4).collect());
/// assert_eq!(result, vec![(Some(1), None, None, None)]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::map_latest4;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let s4 = stream::iter([0.5_f64]);
/// let result: Vec<_> = block_on(map_latest4(s1, s2, s3, s4, |a, b, c, d| {
///     format!("{a}-{b}-{c}-{d}")
/// }).collect());
/// assert_eq!(result, vec!["1-a-true-0.5"]);
/// ```
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
///
/// # Example
///
/// ```
/// use combine_latest::map_latest_opt4;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let s1 = stream::iter([1]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let s4 = stream::iter([0.5_f64]);
/// let result: Vec<_> = block_on(map_latest_opt4(s1, s2, s3, s4, |a, b, c, d| {
///     (a.copied(), b.copied(), c.copied(), d.copied())
/// }).collect());
/// assert_eq!(result[0], (Some(1), None, None, None));
/// ```
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

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor::block_on;
    use futures::stream::{self, StreamExt};

    // Tests rely on `futures::stream::select` round-robin polling: with immediately-ready
    // `stream::iter` streams, it alternates s1, s2, s1, s2, ... yielding deterministic output.

    // -- 2-stream tests --

    #[test]
    fn combine_latest_basic() {
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> = block_on(combine_latest(s1, s2).collect());
        // select round-robins: s1(1), s2("a"), s1(2), s2("b"), s1(3)
        // After s1(1): (Some(1), None) → filtered
        // After s2("a"): (1, "a")
        // After s1(2): (2, "a")
        // After s2("b"): (2, "b")
        // After s1(3): (3, "b")
        assert_eq!(result, vec![(1, "a"), (2, "a"), (2, "b"), (3, "b")]);
    }

    #[test]
    fn combine_latest_empty_stream() {
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(Vec::<&str>::new());
        let result: Vec<_> = block_on(combine_latest(s1, s2).collect());
        assert!(result.is_empty());
    }

    #[test]
    fn combine_latest_opt_basic() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(["a"]);
        let result: Vec<_> = block_on(combine_latest_opt(s1, s2).collect());
        assert_eq!(
            result,
            vec![(Some(1), None), (Some(1), Some("a")), (Some(2), Some("a")),]
        );
    }

    #[test]
    fn combine_latest_opt_empty_stream() {
        let s1 = stream::iter(Vec::<i32>::new());
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> = block_on(combine_latest_opt(s1, s2).collect());
        assert_eq!(result, vec![(None, Some("a")), (None, Some("b"))]);
    }

    #[test]
    #[allow(deprecated)]
    fn combine_latest_optional_delegates() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let result: Vec<_> = block_on(combine_latest_optional(s1, s2).collect());
        let expected: Vec<_> =
            block_on(combine_latest_opt(stream::iter([1]), stream::iter(["a"])).collect());
        assert_eq!(result, expected);
    }

    #[test]
    fn map_latest_basic() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> = block_on(map_latest(s1, s2, |n, s| format!("{n}{s}")).collect());
        assert_eq!(result, vec!["1a", "2a", "2b"]);
    }

    #[test]
    fn map_latest_non_clone() {
        // String is Clone, but this demonstrates the API works with references
        struct NonClone(i32);
        let s1 = stream::iter([NonClone(1), NonClone(2)]);
        let s2 = stream::iter([NonClone(10)]);
        let result: Vec<_> = block_on(map_latest(s1, s2, |a, b| a.0 + b.0).collect());
        assert_eq!(result, vec![11, 12]);
    }

    #[test]
    fn map_latest_opt_basic() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(["a"]);
        let result: Vec<_> =
            block_on(map_latest_opt(s1, s2, |n, s| (n.copied(), s.copied())).collect());
        assert_eq!(
            result,
            vec![(Some(1), None), (Some(1), Some("a")), (Some(2), Some("a")),]
        );
    }

    // -- 3-stream tests --

    #[test]
    fn combine_latest3_basic() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let result: Vec<_> = block_on(combine_latest3(s1, s2, s3).collect());
        // Outer select round-robins inner(s1,s2) and s3:
        //   inner yields s1(1), then outer yields s3(true), then inner yields s2("a"), ...
        // After s1(1): c=(Some(1), None, None) → filtered
        // After s3(true): c=(Some(1), None, Some(true)) → filtered
        // After s2("a"): c=(Some(1), Some("a"), Some(true)) → (1, "a", true)
        // After s1(2): c=(Some(2), Some("a"), Some(true)) → (2, "a", true)
        assert_eq!(result, vec![(1, "a", true), (2, "a", true)]);
    }

    #[test]
    fn combine_latest3_empty_stream() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(Vec::<&str>::new());
        let s3 = stream::iter([true]);
        let result: Vec<_> = block_on(combine_latest3(s1, s2, s3).collect());
        assert!(result.is_empty());
    }

    #[test]
    fn combine_latest_opt3_basic() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter::<[bool; 0]>([]);
        let result: Vec<_> = block_on(combine_latest_opt3(s1, s2, s3).collect());
        assert_eq!(
            result,
            vec![(Some(1), None, None), (Some(1), Some("a"), None),]
        );
    }

    #[test]
    fn map_latest3_basic() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let result: Vec<_> =
            block_on(map_latest3(s1, s2, s3, |n, s, b| format!("{n}-{s}-{b}")).collect());
        assert_eq!(result, vec!["1-a-true", "2-a-true"]);
    }

    #[test]
    fn map_latest_opt3_basic() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let result: Vec<_> = block_on(
            map_latest_opt3(s1, s2, s3, |n, s, b| (n.copied(), s.copied(), b.copied())).collect(),
        );
        assert_eq!(result[0], (Some(1), None, None));
        assert!(result.len() >= 3);
    }

    // -- 4-stream tests --

    #[test]
    fn combine_latest4_basic() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let s4 = stream::iter([0.5]);
        let result: Vec<_> = block_on(combine_latest4(s1, s2, s3, s4).collect());
        assert_eq!(result, vec![(1, "a", true, 0.5)]);
    }

    #[test]
    fn combine_latest4_empty_stream() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter(Vec::<bool>::new());
        let s4 = stream::iter([0.5]);
        let result: Vec<_> = block_on(combine_latest4(s1, s2, s3, s4).collect());
        assert!(result.is_empty());
    }

    #[test]
    fn combine_latest_opt4_basic() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter::<[&str; 0]>([]);
        let s3 = stream::iter::<[bool; 0]>([]);
        let s4 = stream::iter::<[f64; 0]>([]);
        let result: Vec<_> = block_on(combine_latest_opt4(s1, s2, s3, s4).collect());
        assert_eq!(result, vec![(Some(1), None, None, None)]);
    }

    #[test]
    fn map_latest4_basic() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let s4 = stream::iter([0.5_f64]);
        let result: Vec<_> = block_on(
            map_latest4(s1, s2, s3, s4, |a, b, c, d| format!("{a}-{b}-{c}-{d}")).collect(),
        );
        assert_eq!(result, vec!["1-a-true-0.5"]);
    }

    #[test]
    fn map_latest_opt4_basic() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let s4 = stream::iter([0.5_f64]);
        let result: Vec<_> = block_on(
            map_latest_opt4(s1, s2, s3, s4, |a, b, c, d| {
                (a.copied(), b.copied(), c.copied(), d.copied())
            })
            .collect(),
        );
        assert_eq!(result[0], (Some(1), None, None, None));
    }
}
