use core::marker::PhantomData;
use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::Serialize;

/// Finite set, containing all its values.
pub trait FiniteSet<const N: usize>: Copy + Sized {
    const ALL: [Self; N];
    const LEN: usize = N;

    #[inline]
    fn iter() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter()
    }

    #[inline]
    fn iter_rev() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().rev()
    }
}

pub trait Empty {
    const EMPTY: Self;

    fn is_empty(&self) -> bool;
}

impl Empty for u8 {
    const EMPTY: Self = 0;

    fn is_empty(&self) -> bool {
        *self == Self::EMPTY
    }
}

impl Empty for u128 {
    const EMPTY: Self = 0;

    fn is_empty(&self) -> bool {
        *self == Self::EMPTY
    }
}

impl<T> Empty for Option<T> {
    const EMPTY: Self = None;

    fn is_empty(&self) -> bool {
        self.is_none()
    }
}

impl<T> Empty for Vec<T> {
    const EMPTY: Self = Vec::new();

    fn is_empty(&self) -> bool {
        self.is_empty()
    }
}

/// "Full" table, containing an element of `T` for every value of `X`.
#[derive(Clone, Copy, Debug)]
pub struct Table<X, T, const N: usize> {
    pub all: [T; N],
    __: PhantomData<X>,
}

impl<X, T, const N: usize> Table<X, T, N> {
    #[inline]
    pub const fn new(all: [T; N]) -> Self {
        Self { all, __: PhantomData }
    }
}

impl<X, T: Copy, const N: usize> Table<X, T, N> {
    #[inline]
    pub const fn filled(value: T) -> Self {
        Self { all: [value; N], __: PhantomData }
    }
}

impl<X, T: Empty, const N: usize> Empty for Table<X, T, N> {
    const EMPTY: Self = Self::new([const { T::EMPTY }; N]);

    fn is_empty(&self) -> bool {
        self.all.iter().all(Empty::is_empty)
    }
}

impl<X, T: Empty, const N: usize> Table<X, T, N> {
    #[inline]
    pub const fn empty() -> Self {
        Self::EMPTY
    }
}

impl<X, T: Default, const N: usize> Default for Table<X, T, N>
where
    [T; N]: Default,
{
    fn default() -> Self {
        Self { all: Default::default(), __: PhantomData }
    }
}

impl<X, T, const N: usize> From<Table<X, T, N>> for BTreeMap<X, T>
where
    T: Copy,
    X: FiniteSet<N> + Ord,
{
    fn from(table: Table<X, T, N>) -> Self {
        X::iter().zip(table.all).collect()
    }
}

#[cfg(feature = "serde")]
impl<X, T: Serialize, const N: usize> Serialize for Table<X, T, N>
where
    T: Copy + Serialize,
    X: FiniteSet<N> + Ord + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&BTreeMap::from(*self), serializer)
    }
}

/// Defines a small finite enum, its table type, and optionally its record type.
///
/// This macro keeps enum-indexed tables usable in const contexts. It works
/// around current Rust const-fn limitations by generating inherent const
/// helpers instead of relying only on trait methods and indexing operators.
///
/// Every variant must have an `as label`. The label becomes the canonical
/// string returned by `name()` and written by `Display`.
///
/// Table-only form:
///
/// ```ignore
/// finite_set!(
///     /// Board square.
///     Square,
///     /// Array-backed table keyed by square.
///     SquareTable {
///         /// Lower-left square from White's perspective.
///         A1 = 0 as a1,
///         B1 = 1 as b1,
///     }
/// );
/// ```
///
/// Table-only form with cursor:
///
/// ```ignore
/// finite_set!(
///     File,
///     FileTable {
///         A = 0 as a,
///         B = 1 as b,
///     },
///     FileCursor
/// );
/// ```
///
/// Attributes are optional at every documented position. Values are optional
/// when the previous variant's discriminant plus one is the desired value:
///
/// ```ignore
/// finite_set!(Square, SquareTable {
///     A1 = 0 as a1,
///     B1 as b1,
/// });
/// ```
///
/// Record form:
///
/// ```ignore
/// finite_set!(
///     /// Player to move / piece owner.
///     Player,
///     /// Named-field values keyed by player.
///     Players,
///     /// Array-backed table keyed by player.
///     PlayerTable,
///     {
///         /// Black player.
///         Black = 0 as black,
///         /// White player.
///         White = 1 as white,
///     }
/// );
/// ```
///
/// The generated enum gets:
/// - `ALL` and `LEN`
/// - `name`, `eq`, `index`, `from_index`, and `panicky_from_index`
/// - `iter` and `iter_rev`
/// - `Display` via `name`
///
/// The generated table type is an alias for [`Table`] and gets key-based
/// indexing plus `get`, `get_ref`, and `get_mut`.
///
/// The record form also generates a named-field struct with the same key-based
/// indexing and accessors.
///
/// The public arms calculate the enum length with `@count`, then dispatch to
/// private arms. `@table` emits the enum, `FiniteSet`, and the array-backed
/// table alias. `@record` first delegates to `@table`, then adds the named-field
/// record and its indexing helpers.
#[macro_export]
macro_rules! finite_set {
    (
        $(#[$name_meta:meta])*
        $name:ident,
        $(#[$table_meta:meta])*
        $table:ident {
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)? as $field:tt),+ $(,)?
        }
    ) => {
        $crate::finite_set! {
            @table
            { $crate::finite_set!(@count $($variant),+) },
            { $(#[$name_meta])* },
            $name,
            { $(#[$table_meta])* },
            $table,
            $table {
                $($(#[$variant_meta])* $variant $(= $value)? as $field),+
            }
        }
    };

    (
        $(#[$name_meta:meta])*
        $name:ident,
        $(#[$table_meta:meta])*
        $table:ident {
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)? as $field:tt),+ $(,)?
        },
        $cursor:ident
    ) => {
        $crate::finite_set! {
            @table
            { $crate::finite_set!(@count $($variant),+) },
            { $(#[$name_meta])* },
            $name,
            { $(#[$table_meta])* },
            $table,
            $table {
                $($(#[$variant_meta])* $variant $(= $value)? as $field),+
            }
        }

        $crate::finite_set! {
            @cursor
            { $crate::finite_set!(@count $($variant),+) },
            $name,
            $cursor
        }
    };

    (
        $(#[$name_meta:meta])*
        $name:ident,
        $(#[$record_meta:meta])*
        $record:ident,
        $(#[$table_meta:meta])*
        $table:ident,
        {
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)? as $field:ident),+ $(,)?
        }
    ) => {
        $crate::finite_set! {
            @record
            { $crate::finite_set!(@count $($variant),+) },
            { $(#[$name_meta])* },
            $name,
            { $(#[$record_meta])* },
            $record,
            { $(#[$table_meta])* },
            $table,
            {
                $($(#[$variant_meta])* $variant $(= $value)? as $field),+
            }
        }
    };

    (
        @table
        $len:expr,
        { $(#[$name_meta:meta])* },
        $name:ident,
        { $(#[$table_meta:meta])* },
        $table:ident,
        $table_expr:ident {
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)? as $field:tt),+ $(,)?
        }
    ) => {
        $(#[$name_meta])*
        #[repr(u8)]
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        #[cfg_attr(feature = "serde", derive(SerializeDisplay))]
        pub enum $name {
            $(
                $(#[$variant_meta])*
                $variant $(= $value)?
            ),+
        }

        $(#[$table_meta])*
        pub type $table<T = bool> = $crate::finite::Table<$name, T, $len>;

        impl $crate::finite::FiniteSet<$len> for $name {
            const ALL: [Self; $len] = [$(Self::$variant),+];
        }

        #[doc = concat!(stringify!($name), " Finite Set API.")]
        impl $name {
            pub const LEN: usize = $len;

            pub const ALL: [Self; $len] = <Self as $crate::finite::FiniteSet<$len>>::ALL;

            #[inline]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => $crate::finite_set!(@name $field)),+
                }
            }

            #[inline]
            pub const fn eq(self, other: Self) -> bool {
                self as u8 == other as u8
            }

            #[inline]
            pub const fn index(self) -> usize {
                let mut index = 0;
                while index < Self::ALL.len() {
                    if Self::ALL[index] as u8 == self as u8 {
                        return index;
                    }
                    index += 1;
                }
                unreachable!()
            }

            #[inline]
            pub const fn from_index(index: u8) -> Option<Self> {
                if index < Self::LEN as u8 {
                    Some(Self::panicky_from_index(index))
                } else {
                    None
                }
            }

            #[inline]
            #[track_caller]
            pub(crate) const fn panicky_from_index(index: u8) -> Self {
                assert!(index < Self::LEN as u8);
                Self::ALL[index as usize]
            }

            #[inline]
            pub fn iter() -> impl Iterator<Item = Self> {
                <Self as $crate::finite::FiniteSet<$len>>::iter()
            }

            #[inline]
            pub fn iter_rev() -> impl Iterator<Item = Self> {
                <Self as $crate::finite::FiniteSet<$len>>::iter_rev()
            }
        }

        impl core::fmt::Display for $name {
            #[inline]
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str(self.name())
            }
        }

        impl<T: Copy> $crate::finite::Table<$name, T, $len> {
            #[inline]
            pub const fn get(&self, key: $name) -> T {
                self.all[key.index()]
            }

            #[inline]
            pub const fn set(&mut self, key: $name, value: T) {
                self.all[key.index()] = value;
            }
        }

        impl<T> $crate::finite::Table<$name, T, $len> {
            #[inline]
            pub const fn get_ref(&self, key: $name) -> &T {
                &self.all[key.index()]
            }

            #[inline]
            pub const fn get_mut(&mut self, key: $name) -> &mut T {
                &mut self.all[key.index()]
            }
        }

        impl<T> core::ops::Index<$name> for $crate::finite::Table<$name, T, $len> {
            type Output = T;

            #[inline]
            fn index(&self, key: $name) -> &T {
                &self.all[key.index()]
            }
        }

        impl<T> core::ops::IndexMut<$name> for $crate::finite::Table<$name, T, $len> {
            #[inline]
            fn index_mut(&mut self, key: $name) -> &mut T {
                &mut self.all[key.index()]
            }
        }
    };

    (
        @record
        $len:expr,
        { $(#[$name_meta:meta])* },
        $name:ident,
        { $(#[$record_meta:meta])* },
        $record:ident,
        { $(#[$table_meta:meta])* },
        $table:ident,
        {
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)? as $field:ident),+ $(,)?
        }
    ) => {
        $crate::finite_set! {
            @table
            $len,
            { $(#[$name_meta])* },
            $name,
            { $(#[$table_meta])* },
            $table,
            $table {
                $($(#[$variant_meta])* $variant $(= $value)? as $field),+
            }
        }

        $(#[$record_meta])*
        #[derive(Clone, Copy, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $record<T = bool> {
            $(
                $(#[$variant_meta])*
                pub $field: T
            ),+
        }

        impl<T: Copy> $record<T> {
            #[inline]
            pub const fn get(&self, key: $name) -> T {
                *self.get_ref(key)
            }
        }

        impl<T> $record<T> {
            #[inline]
            pub const fn get_ref(&self, key: $name) -> &T {
                match key {
                    $( $name::$variant => &self.$field ),+
                }
            }

            #[inline]
            pub const fn get_mut(&mut self, key: $name) -> &mut T {
                match key {
                    $( $name::$variant => &mut self.$field ),+
                }
            }
        }

        impl<T: $crate::finite::Empty> $crate::finite::Empty for $record<T> {
            const EMPTY: Self = Self {
                $( $field: T::EMPTY ),+
            };

            fn is_empty(&self) -> bool {
                $( self.$field.is_empty() )&&+
            }
        }

        impl<T> core::ops::Index<$name> for $record<T> {
            type Output = T;

            #[inline]
            fn index(&self, key: $name) -> &T {
                self.get_ref(key)
            }
        }

        impl<T> core::ops::IndexMut<$name> for $record<T> {
            #[inline]
            fn index_mut(&mut self, key: $name) -> &mut T {
                self.get_mut(key)
            }
        }
    };

    (
        @cursor
        $len:expr,
        $name:ident,
        $cursor:ident
    ) => {
        #[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
        pub struct $cursor {
            next: u8,
        }

        impl $name {
            #[inline]
            pub const fn cursor() -> $cursor {
                $cursor { next: 0 }
            }
        }

        impl $cursor {
            #[inline]
            pub const fn next(&mut self) -> Option<$name> {
                if self.done() {
                    None
                } else {
                    let value = $name::panicky_from_index(self.next);
                    self.next += 1;
                    Some(value)
                }
            }

            #[inline]
            pub const fn skip(&mut self, n: u8) -> bool {
                let Some(next) = self.next.checked_add(n) else {
                    return false;
                };
                if next > $len as u8 {
                    return false;
                }
                self.next = next;
                true
            }

            #[inline]
            pub const fn done(self) -> bool {
                self.next >= $len as u8
            }
        }
    };

    (@count $($variant:ident),+) => {
        <[()]>::len(&[$($crate::finite_set!(@unit $variant)),+])
    };

    (@unit $variant:ident) => {
        ()
    };

    (@name $field:ident) => {
        stringify!($field)
    };

    (@name $field:literal) => {
        $field
    };
}

#[macro_export]
/// Loops over all values of a generated finite set in const contexts.
///
/// This is a small const-friendly replacement for `for value in Type::ALL`.
macro_rules! finite_for {
    ($value:ident in $set:ty $body:block) => {{
        let all = <$set>::ALL;
        let mut index = 0usize;
        while index < all.len() {
            let $value = all[index];
            $body
            index += 1;
        }
    }};
}

// TODO: If we ever need it, add a finite_map! and finite_find! etc.
