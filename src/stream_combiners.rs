use std::future;

use futures_core::Stream;
use futures_util::{StreamExt, stream::select};

// ---------------------------------------------------------------------------
// Traits
// ---------------------------------------------------------------------------

/// Combines multiple streams into a single stream that yields tuples with the latest value from
/// each input stream. The output stream won't yield until every input stream has produced at least
/// one item. Implemented for tuples of 2 to 12 streams whose items implement `Clone`.
pub trait CombineLatest {
    type Item;
    fn combine_latest(self) -> impl Stream<Item = Self::Item>;
}

/// Like [`CombineLatest`], but wraps each position in `Option` and starts yielding as soon as
/// *any* input stream produces a value. Positions that haven't yielded yet are `None`.
/// Implemented for tuples of 2 to 12 streams whose items implement `Clone`.
pub trait CombineLatestOpt {
    type Item;
    fn combine_latest_opt(self) -> impl Stream<Item = Self::Item>;
}

/// Combines multiple streams and applies a closure to references of the latest values. The output
/// stream won't yield until every input stream has produced at least one item. Unlike
/// [`CombineLatest`], the item types don't need to implement `Clone`. Implemented for tuples of
/// 2 to 12 streams.
pub trait MapLatest<F> {
    type Output;
    fn map_latest(self, f: F) -> impl Stream<Item = Self::Output>;
}

/// Like [`MapLatest`], but the closure receives `Option` references and the output stream starts
/// yielding as soon as *any* input stream produces a value. Implemented for tuples of 2 to 12
/// streams.
pub trait MapLatestOpt<F> {
    type Output;
    fn map_latest_opt(self, f: F) -> impl Stream<Item = Self::Output>;
}

/// Emits the latest values from all streams as a tuple, but **only when the primary (first)
/// stream yields**. Secondary streams silently update their cached values without triggering
/// output. No output is produced until every secondary stream has yielded at least once.
/// Implemented for tuples of 2 to 12 streams whose items implement `Clone`.
pub trait WithLatestFrom {
    type Item;
    fn with_latest_from(self) -> impl Stream<Item = Self::Item>;
}

/// Like [`WithLatestFrom`], but applies a closure to references of the latest values instead of
/// returning a tuple. The item types don't need to implement `Clone`. Implemented for tuples of
/// 2 to 12 streams.
pub trait MapWithLatestFrom<F> {
    type Output;
    fn map_with_latest_from(self, f: F) -> impl Stream<Item = Self::Output>;
}

// ---------------------------------------------------------------------------
// Helper macro – left-fold nested select
// ---------------------------------------------------------------------------

macro_rules! nest_select {
    ($a:expr, $b:expr) => { select($a, $b) };
    ($a:expr, $b:expr, $($rest:expr),+) => {
        nest_select!(select($a, $b), $($rest),+)
    };
}

// ---------------------------------------------------------------------------
// Main macro – generates tag enum + all 4 trait impls for one arity
// ---------------------------------------------------------------------------

macro_rules! impl_combine_latest {
    (
        $Tag:ident { $($Var:ident),+ }
        [$(($T:ident, $S:ident, $s:ident, $c:ident, $lt:lifetime)),+]
    ) => {
        enum $Tag<$($T),+> { $($Var($T)),+ }

        impl<$($T: Clone, $S: Stream<Item = $T>),+> CombineLatest for ($($S,)+) {
            type Item = ($($T,)+);

            fn combine_latest(self) -> impl Stream<Item = Self::Item> {
                let ($($s,)+) = self;
                $(let mut $c: Option<$T> = None;)+
                nest_select!($($s.map($Tag::$Var)),+)
                    .filter_map(move |tagged| {
                        match tagged { $($Tag::$Var(v) => $c = Some(v),)+ }
                        future::ready(match ($($c.clone(),)+) {
                            ($(Some($s),)+) => Some(($($s,)+)),
                            _ => None,
                        })
                    })
            }
        }

        impl<$($T: Clone, $S: Stream<Item = $T>),+> CombineLatestOpt for ($($S,)+) {
            type Item = ($(Option<$T>,)+);

            fn combine_latest_opt(self) -> impl Stream<Item = Self::Item> {
                let ($($s,)+) = self;
                $(let mut $c: Option<$T> = None;)+
                nest_select!($($s.map($Tag::$Var)),+)
                    .map(move |tagged| {
                        match tagged { $($Tag::$Var(v) => $c = Some(v),)+ }
                        ($($c.clone(),)+)
                    })
            }
        }

        impl<$($T, $S: Stream<Item = $T>,)+ U, F> MapLatest<F> for ($($S,)+)
        where
            F: for<$($lt),+> FnMut($(&$lt $T),+) -> U,
        {
            type Output = U;

            fn map_latest(self, mut f: F) -> impl Stream<Item = U> {
                let ($($s,)+) = self;
                $(let mut $c: Option<$T> = None;)+
                nest_select!($($s.map($Tag::$Var)),+)
                    .filter_map(move |tagged| {
                        match tagged { $($Tag::$Var(v) => $c = Some(v),)+ }
                        future::ready(match ($($c.as_ref(),)+) {
                            ($(Some($s),)+) => Some(f($($s),+)),
                            _ => None,
                        })
                    })
            }
        }

        impl<$($T, $S: Stream<Item = $T>,)+ U, F> MapLatestOpt<F> for ($($S,)+)
        where
            F: for<$($lt),+> FnMut($(Option<&$lt $T>),+) -> U,
        {
            type Output = U;

            fn map_latest_opt(self, mut f: F) -> impl Stream<Item = U> {
                let ($($s,)+) = self;
                $(let mut $c: Option<$T> = None;)+
                nest_select!($($s.map($Tag::$Var)),+)
                    .map(move |tagged| {
                        match tagged { $($Tag::$Var(v) => $c = Some(v),)+ }
                        f($($c.as_ref(),)+)
                    })
            }
        }
    };
}

// ---------------------------------------------------------------------------
// with_latest_from macro – emits only when the primary (first) stream yields
// ---------------------------------------------------------------------------

macro_rules! impl_with_latest_from {
    (
        $Tag:ident { $PrimaryVar:ident $(, $SecondaryVar:ident)+ }
        [($PT:ident, $PS:ident, $ps:ident, $pc:ident, $plt:lifetime)
         $(, ($T:ident, $S:ident, $s:ident, $c:ident, $lt:lifetime))+]
    ) => {
        impl<$PT: Clone, $($T: Clone,)+ $PS: Stream<Item = $PT>, $($S: Stream<Item = $T>),+>
            WithLatestFrom for ($PS, $($S,)+)
        {
            type Item = ($PT, $($T,)+);

            fn with_latest_from(self) -> impl Stream<Item = Self::Item> {
                let ($ps, $($s,)+) = self;
                let mut $pc: Option<$PT> = None;
                $(let mut $c: Option<$T> = None;)+
                nest_select!($ps.map($Tag::$PrimaryVar), $($s.map($Tag::$SecondaryVar)),+)
                    .filter_map(move |tagged| {
                        let is_primary = matches!(tagged, $Tag::$PrimaryVar(_));
                        match tagged {
                            $Tag::$PrimaryVar(v) => $pc = Some(v),
                            $($Tag::$SecondaryVar(v) => $c = Some(v),)+
                        }
                        future::ready(if is_primary {
                            match ($pc.clone(), $($c.clone(),)+) {
                                (Some($ps), $(Some($s),)+) => Some(($ps, $($s,)+)),
                                _ => None,
                            }
                        } else {
                            None
                        })
                    })
            }
        }

        impl<$PT, $($T,)+ $PS: Stream<Item = $PT>, $($S: Stream<Item = $T>,)+ U, F>
            MapWithLatestFrom<F> for ($PS, $($S,)+)
        where
            F: for<$plt, $($lt),+> FnMut(&$plt $PT, $(&$lt $T),+) -> U,
        {
            type Output = U;

            fn map_with_latest_from(self, mut f: F) -> impl Stream<Item = U> {
                let ($ps, $($s,)+) = self;
                let mut $pc: Option<$PT> = None;
                $(let mut $c: Option<$T> = None;)+
                nest_select!($ps.map($Tag::$PrimaryVar), $($s.map($Tag::$SecondaryVar)),+)
                    .filter_map(move |tagged| {
                        let is_primary = matches!(tagged, $Tag::$PrimaryVar(_));
                        match tagged {
                            $Tag::$PrimaryVar(v) => $pc = Some(v),
                            $($Tag::$SecondaryVar(v) => $c = Some(v),)+
                        }
                        future::ready(if is_primary {
                            match ($pc.as_ref(), $($c.as_ref(),)+) {
                                (Some($ps), $(Some($s),)+) => Some(f($ps, $($s),+)),
                                _ => None,
                            }
                        } else {
                            None
                        })
                    })
            }
        }
    };
}

// ---------------------------------------------------------------------------
// Macro invocations – one per arity (2 through 12)
// ---------------------------------------------------------------------------

impl_combine_latest!(LR { L, R }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b)]
);

impl_combine_latest!(Tag3 { A, B, C }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c)]
);

impl_combine_latest!(Tag4 { A, B, C, D }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd)]
);

impl_combine_latest!(Tag5 { A, B, C, D, E }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e)]
);

impl_combine_latest!(Tag6 { A, B, C, D, E, F }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f)]
);

impl_combine_latest!(Tag7 { A, B, C, D, E, F, G }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g)]
);

impl_combine_latest!(Tag8 { A, B, C, D, E, F, G, H }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h)]
);

impl_combine_latest!(Tag9 { A, B, C, D, E, F, G, H, I }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i)]
);

impl_combine_latest!(Tag10 { A, B, C, D, E, F, G, H, I, J }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i), (T10, S10, s10, c10, 'j)]
);

impl_combine_latest!(Tag11 { A, B, C, D, E, F, G, H, I, J, K }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i), (T10, S10, s10, c10, 'j), (T11, S11, s11, c11, 'k)]
);

impl_combine_latest!(Tag12 { A, B, C, D, E, F, G, H, I, J, K, L }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i), (T10, S10, s10, c10, 'j), (T11, S11, s11, c11, 'k),
     (T12, S12, s12, c12, 'l)]
);

impl_with_latest_from!(LR { L, R }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b)]
);

impl_with_latest_from!(Tag3 { A, B, C }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c)]
);

impl_with_latest_from!(Tag4 { A, B, C, D }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd)]
);

impl_with_latest_from!(Tag5 { A, B, C, D, E }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e)]
);

impl_with_latest_from!(Tag6 { A, B, C, D, E, F }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f)]
);

impl_with_latest_from!(Tag7 { A, B, C, D, E, F, G }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g)]
);

impl_with_latest_from!(Tag8 { A, B, C, D, E, F, G, H }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h)]
);

impl_with_latest_from!(Tag9 { A, B, C, D, E, F, G, H, I }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i)]
);

impl_with_latest_from!(Tag10 { A, B, C, D, E, F, G, H, I, J }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i), (T10, S10, s10, c10, 'j)]
);

impl_with_latest_from!(Tag11 { A, B, C, D, E, F, G, H, I, J, K }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i), (T10, S10, s10, c10, 'j), (T11, S11, s11, c11, 'k)]
);

impl_with_latest_from!(Tag12 { A, B, C, D, E, F, G, H, I, J, K, L }
    [(T1, S1, s1, c1, 'a), (T2, S2, s2, c2, 'b), (T3, S3, s3, c3, 'c), (T4, S4, s4, c4, 'd),
     (T5, S5, s5, c5, 'e), (T6, S6, s6, c6, 'f), (T7, S7, s7, c7, 'g), (T8, S8, s8, c8, 'h),
     (T9, S9, s9, c9, 'i), (T10, S10, s10, c10, 'j), (T11, S11, s11, c11, 'k),
     (T12, S12, s12, c12, 'l)]
);

// ---------------------------------------------------------------------------
// Public free functions – documented wrappers that delegate to the traits
// ---------------------------------------------------------------------------

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
#[must_use = "streams do nothing unless polled"]
pub fn combine_latest<T1: Clone, T2: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
) -> impl Stream<Item = (T1, T2)> {
    (s1, s2).combine_latest()
}

#[deprecated(since = "1.1.0", note = "Use combine_latest_opt instead")]
#[must_use = "streams do nothing unless polled"]
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
#[must_use = "streams do nothing unless polled"]
pub fn combine_latest_opt<T1: Clone, T2: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
) -> impl Stream<Item = (Option<T1>, Option<T2>)> {
    (s1, s2).combine_latest_opt()
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
#[must_use = "streams do nothing unless polled"]
pub fn map_latest<T1, T2, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    f: impl for<'a, 'b> FnMut(&'a T1, &'b T2) -> U,
) -> impl Stream<Item = U> {
    (s1, s2).map_latest(f)
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
#[must_use = "streams do nothing unless polled"]
pub fn map_latest_opt<T1, T2, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    f: impl for<'a, 'b> FnMut(Option<&'a T1>, Option<&'b T2>) -> U,
) -> impl Stream<Item = U> {
    (s1, s2).map_latest_opt(f)
}

// -- 3-stream free functions --

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
#[must_use = "streams do nothing unless polled"]
pub fn combine_latest3<T1: Clone, T2: Clone, T3: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
) -> impl Stream<Item = (T1, T2, T3)> {
    (s1, s2, s3).combine_latest()
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
#[must_use = "streams do nothing unless polled"]
pub fn combine_latest_opt3<T1: Clone, T2: Clone, T3: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
) -> impl Stream<Item = (Option<T1>, Option<T2>, Option<T3>)> {
    (s1, s2, s3).combine_latest_opt()
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
#[must_use = "streams do nothing unless polled"]
pub fn map_latest3<T1, T2, T3, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    f: impl for<'a, 'b, 'c> FnMut(&'a T1, &'b T2, &'c T3) -> U,
) -> impl Stream<Item = U> {
    (s1, s2, s3).map_latest(f)
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
#[must_use = "streams do nothing unless polled"]
pub fn map_latest_opt3<T1, T2, T3, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    f: impl for<'a, 'b, 'c> FnMut(Option<&'a T1>, Option<&'b T2>, Option<&'c T3>) -> U,
) -> impl Stream<Item = U> {
    (s1, s2, s3).map_latest_opt(f)
}

// -- 4-stream free functions --

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
#[must_use = "streams do nothing unless polled"]
pub fn combine_latest4<T1: Clone, T2: Clone, T3: Clone, T4: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
) -> impl Stream<Item = (T1, T2, T3, T4)> {
    (s1, s2, s3, s4).combine_latest()
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
#[must_use = "streams do nothing unless polled"]
pub fn combine_latest_opt4<T1: Clone, T2: Clone, T3: Clone, T4: Clone>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
) -> impl Stream<Item = (Option<T1>, Option<T2>, Option<T3>, Option<T4>)> {
    (s1, s2, s3, s4).combine_latest_opt()
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
#[must_use = "streams do nothing unless polled"]
pub fn map_latest4<T1, T2, T3, T4, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
    f: impl for<'a, 'b, 'c, 'd> FnMut(&'a T1, &'b T2, &'c T3, &'d T4) -> U,
) -> impl Stream<Item = U> {
    (s1, s2, s3, s4).map_latest(f)
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
#[must_use = "streams do nothing unless polled"]
pub fn map_latest_opt4<T1, T2, T3, T4, U>(
    s1: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
    f: impl for<'a, 'b, 'c, 'd> FnMut(
        Option<&'a T1>,
        Option<&'b T2>,
        Option<&'c T3>,
        Option<&'d T4>,
    ) -> U,
) -> impl Stream<Item = U> {
    (s1, s2, s3, s4).map_latest_opt(f)
}

// -- 2-stream with_latest_from free functions --

/// Emits tuples of the latest values from both streams, but **only when the primary (first)
/// stream yields**. The secondary stream silently updates its cached value without triggering
/// output. No output is produced until the secondary stream has yielded at least once.
///
/// # Example
///
/// ```
/// use combine_latest::with_latest_from;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let clicks = stream::iter([1, 2, 3]);
/// let form_state = stream::iter(["draft"]);
/// let result: Vec<_> = block_on(with_latest_from(clicks, form_state).collect());
/// // Only click 2 and 3 emit — click 1 arrives before form_state has a value
/// assert_eq!(result, vec![(2, "draft"), (3, "draft")]);
/// ```
#[must_use = "streams do nothing unless polled"]
pub fn with_latest_from<T1: Clone, T2: Clone>(
    primary: impl Stream<Item = T1>,
    secondary: impl Stream<Item = T2>,
) -> impl Stream<Item = (T1, T2)> {
    (primary, secondary).with_latest_from()
}

/// Emits values from two streams by applying a function, but **only when the primary (first)
/// stream yields**. The function takes references, so the types don't need to implement `Clone`.
///
/// # Example
///
/// ```
/// use combine_latest::map_with_latest_from;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let clicks = stream::iter([1, 2, 3]);
/// let label = stream::iter(["save"]);
/// let result: Vec<_> = block_on(
///     map_with_latest_from(clicks, label, |n, s| format!("{s}:{n}")).collect()
/// );
/// assert_eq!(result, vec!["save:2", "save:3"]);
/// ```
#[must_use = "streams do nothing unless polled"]
pub fn map_with_latest_from<T1, T2, U>(
    primary: impl Stream<Item = T1>,
    secondary: impl Stream<Item = T2>,
    f: impl for<'a, 'b> FnMut(&'a T1, &'b T2) -> U,
) -> impl Stream<Item = U> {
    (primary, secondary).map_with_latest_from(f)
}

// -- 3-stream with_latest_from free functions --

/// Emits tuples of the latest values from three streams, but **only when the primary (first)
/// stream yields**. See [`with_latest_from`] for details.
///
/// # Example
///
/// ```
/// use combine_latest::with_latest_from3;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let primary = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let result: Vec<_> = block_on(with_latest_from3(primary, s2, s3).collect());
/// assert_eq!(result, vec![(2, "a", true)]);
/// ```
#[must_use = "streams do nothing unless polled"]
pub fn with_latest_from3<T1: Clone, T2: Clone, T3: Clone>(
    primary: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
) -> impl Stream<Item = (T1, T2, T3)> {
    (primary, s2, s3).with_latest_from()
}

/// Emits values from three streams by applying a function, but **only when the primary (first)
/// stream yields**. See [`map_with_latest_from`] for details.
///
/// # Example
///
/// ```
/// use combine_latest::map_with_latest_from3;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let primary = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let result: Vec<_> = block_on(map_with_latest_from3(primary, s2, s3, |n, s, b| {
///     format!("{n}-{s}-{b}")
/// }).collect());
/// assert_eq!(result, vec!["2-a-true"]);
/// ```
#[must_use = "streams do nothing unless polled"]
pub fn map_with_latest_from3<T1, T2, T3, U>(
    primary: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    f: impl for<'a, 'b, 'c> FnMut(&'a T1, &'b T2, &'c T3) -> U,
) -> impl Stream<Item = U> {
    (primary, s2, s3).map_with_latest_from(f)
}

// -- 4-stream with_latest_from free functions --

/// Emits tuples of the latest values from four streams, but **only when the primary (first)
/// stream yields**. See [`with_latest_from`] for details.
///
/// # Example
///
/// ```
/// use combine_latest::with_latest_from4;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let primary = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let s4 = stream::iter([0.5]);
/// let result: Vec<_> = block_on(with_latest_from4(primary, s2, s3, s4).collect());
/// assert_eq!(result, vec![(2, "a", true, 0.5)]);
/// ```
#[must_use = "streams do nothing unless polled"]
pub fn with_latest_from4<T1: Clone, T2: Clone, T3: Clone, T4: Clone>(
    primary: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
) -> impl Stream<Item = (T1, T2, T3, T4)> {
    (primary, s2, s3, s4).with_latest_from()
}

/// Emits values from four streams by applying a function, but **only when the primary (first)
/// stream yields**. See [`map_with_latest_from`] for details.
///
/// # Example
///
/// ```
/// use combine_latest::map_with_latest_from4;
/// use futures::executor::block_on;
/// use futures::stream::{self, StreamExt};
///
/// let primary = stream::iter([1, 2]);
/// let s2 = stream::iter(["a"]);
/// let s3 = stream::iter([true]);
/// let s4 = stream::iter([0.5_f64]);
/// let result: Vec<_> = block_on(map_with_latest_from4(primary, s2, s3, s4, |a, b, c, d| {
///     format!("{a}-{b}-{c}-{d}")
/// }).collect());
/// assert_eq!(result, vec!["2-a-true-0.5"]);
/// ```
#[must_use = "streams do nothing unless polled"]
pub fn map_with_latest_from4<T1, T2, T3, T4, U>(
    primary: impl Stream<Item = T1>,
    s2: impl Stream<Item = T2>,
    s3: impl Stream<Item = T3>,
    s4: impl Stream<Item = T4>,
    f: impl for<'a, 'b, 'c, 'd> FnMut(&'a T1, &'b T2, &'c T3, &'d T4) -> U,
) -> impl Stream<Item = U> {
    (primary, s2, s3, s4).map_with_latest_from(f)
}

// For 5+ streams, use the trait-based API directly:
//   (s1, s2, s3, s4, s5).combine_latest()
//   (s1, s2, s3, s4, s5).with_latest_from()
//   (s1, s2, s3, s4, s5).map_latest(|a, b, c, d, e| ...)

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

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

    // -- trait-based API tests --

    #[test]
    fn trait_combine_latest_2() {
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> = block_on((s1, s2).combine_latest().collect());
        assert_eq!(result, vec![(1, "a"), (2, "a"), (2, "b"), (3, "b")]);
    }

    #[test]
    fn trait_combine_latest_opt_3() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter::<[bool; 0]>([]);
        let result: Vec<_> = block_on((s1, s2, s3).combine_latest_opt().collect());
        assert_eq!(
            result,
            vec![(Some(1), None, None), (Some(1), Some("a"), None),]
        );
    }

    #[test]
    fn trait_map_latest_4() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let s4 = stream::iter([0.5_f64]);
        let result: Vec<_> = block_on(
            (s1, s2, s3, s4)
                .map_latest(|a, b, c, d| format!("{a}-{b}-{c}-{d}"))
                .collect(),
        );
        assert_eq!(result, vec!["1-a-true-0.5"]);
    }

    #[test]
    fn trait_map_latest_opt_2() {
        let s1 = stream::iter([1, 2]);
        let s2 = stream::iter(["a"]);
        let result: Vec<_> = block_on(
            (s1, s2)
                .map_latest_opt(|n, s| (n.copied(), s.copied()))
                .collect(),
        );
        assert_eq!(
            result,
            vec![(Some(1), None), (Some(1), Some("a")), (Some(2), Some("a")),]
        );
    }

    // -- higher-arity tests (5, 8, 12) --

    #[test]
    fn trait_combine_latest_5() {
        let s1 = stream::iter([1]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let s4 = stream::iter([0.5]);
        let s5 = stream::iter([42u8]);
        let result: Vec<_> = block_on((s1, s2, s3, s4, s5).combine_latest().collect());
        assert_eq!(result, vec![(1, "a", true, 0.5, 42u8)]);
    }

    #[test]
    fn trait_combine_latest_8() {
        let streams = (
            stream::iter([1]),
            stream::iter([2]),
            stream::iter([3]),
            stream::iter([4]),
            stream::iter([5]),
            stream::iter([6]),
            stream::iter([7]),
            stream::iter([8]),
        );
        let result: Vec<_> = block_on(streams.combine_latest().collect());
        assert_eq!(result, vec![(1, 2, 3, 4, 5, 6, 7, 8)]);
    }

    #[test]
    fn trait_combine_latest_12() {
        let streams = (
            stream::iter([1]),
            stream::iter([2]),
            stream::iter([3]),
            stream::iter([4]),
            stream::iter([5]),
            stream::iter([6]),
            stream::iter([7]),
            stream::iter([8]),
            stream::iter([9]),
            stream::iter([10]),
            stream::iter([11]),
            stream::iter([12]),
        );
        let result: Vec<_> = block_on(streams.combine_latest().collect());
        assert_eq!(result, vec![(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12)]);
    }

    #[test]
    fn trait_map_latest_12() {
        let streams = (
            stream::iter([1]),
            stream::iter([2]),
            stream::iter([3]),
            stream::iter([4]),
            stream::iter([5]),
            stream::iter([6]),
            stream::iter([7]),
            stream::iter([8]),
            stream::iter([9]),
            stream::iter([10]),
            stream::iter([11]),
            stream::iter([12]),
        );
        let result: Vec<_> = block_on(
            streams
                .map_latest(|a, b, c, d, e, f, g, h, i, j, k, l| {
                    a + b + c + d + e + f + g + h + i + j + k + l
                })
                .collect(),
        );
        assert_eq!(result, vec![78]);
    }

    // -- with_latest_from tests --

    #[test]
    fn with_latest_from_basic() {
        // Primary: [1, 2, 3], Secondary: ["a", "b"]
        // Round-robin: 1(P), "a"(S), 2(P), "b"(S), 3(P)
        // Emit on primary only when secondary has a value:
        //   1 → secondary not ready yet → skip
        //   "a" → secondary update → no emit
        //   2 → secondary="a" → emit (2, "a")
        //   "b" → secondary update → no emit
        //   3 → secondary="b" → emit (3, "b")
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> = block_on(with_latest_from(s1, s2).collect());
        assert_eq!(result, vec![(2, "a"), (3, "b")]);
    }

    #[test]
    fn with_latest_from_secondary_empty() {
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(Vec::<&str>::new());
        let result: Vec<_> = block_on(with_latest_from(s1, s2).collect());
        assert!(result.is_empty());
    }

    #[test]
    fn with_latest_from_primary_empty() {
        let s1 = stream::iter(Vec::<i32>::new());
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> = block_on(with_latest_from(s1, s2).collect());
        assert!(result.is_empty());
    }

    #[test]
    fn map_with_latest_from_basic() {
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(["a", "b"]);
        let result: Vec<_> =
            block_on(map_with_latest_from(s1, s2, |n, s| format!("{n}{s}")).collect());
        assert_eq!(result, vec!["2a", "3b"]);
    }

    #[test]
    fn map_with_latest_from_non_clone() {
        struct NonClone(i32);
        let s1 = stream::iter([NonClone(1), NonClone(2), NonClone(3)]);
        let s2 = stream::iter([NonClone(10)]);
        let result: Vec<_> = block_on(map_with_latest_from(s1, s2, |a, b| a.0 + b.0).collect());
        assert_eq!(result, vec![12, 13]);
    }

    #[test]
    fn trait_with_latest_from_3() {
        // Primary: [1, 2, 3], s2: ["a"], s3: [true]
        // Round-robin with 3 streams via nested select:
        //   select(select(s1, s2), s3)
        // The primary must emit after all secondaries have emitted.
        let s1 = stream::iter([1, 2, 3]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let result: Vec<_> = block_on((s1, s2, s3).with_latest_from().collect());
        // s1=1, s2="a", s1=2 → emit (2,"a",?) no s3 yet... depends on select ordering
        // With nest_select: select(select(s1,s2), s3)
        // Inner select alternates s1/s2: 1, "a", 2, 3
        // Outer select alternates inner/s3: 1, true, "a", 2, 3
        // So: 1(P, no secondaries), true(S), "a"(S), 2(P, all ready) → (2,"a",true), 3(P) → (3,"a",true)
        assert_eq!(result, vec![(2, "a", true), (3, "a", true)]);
    }

    #[test]
    fn trait_with_latest_from_5() {
        let s1 = stream::iter([1, 2, 3, 4, 5, 6]);
        let s2 = stream::iter(["a"]);
        let s3 = stream::iter([true]);
        let s4 = stream::iter([0.5]);
        let s5 = stream::iter([42u8]);
        let result: Vec<_> = block_on((s1, s2, s3, s4, s5).with_latest_from().collect());
        // The primary must emit after all 4 secondaries have cached values.
        // At minimum, the first few primary values will be skipped.
        assert!(!result.is_empty());
        // All outputs should have the same secondary values since they only have 1 item each
        for (_, s, b, d, u) in &result {
            assert_eq!(*s, "a");
            assert_eq!(*b, true);
            assert_eq!(*d, 0.5);
            assert_eq!(*u, 42u8);
        }
    }
}
