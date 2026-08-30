use core::marker::PhantomData;
use std::collections::BTreeMap;

#[cfg(feature = "serde")]
use serde::Serialize;

/// Finite set, containing all its values.
pub trait FiniteSet<const N: usize>: Copy + Sized {
    const ALL: [Self; N];
    const LEN: usize = N;

    fn iter() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter()
    }

    fn iter_rev() -> impl Iterator<Item = Self> {
        Self::ALL.into_iter().rev()
    }
}

pub trait Empty {
    const EMPTY: Self;
}

impl Empty for u128 {
    const EMPTY: Self = 0;
}

/// "Full" table, containing an element of `T` for every value of `X`.
#[derive(Clone, Copy, Debug)]
pub struct Table<X, T, const N: usize> {
    pub all: [T; N],
    __: PhantomData<X>,
}

impl<X, T, const N: usize> Table<X, T, N> {
    pub const fn new(all: [T; N]) -> Self {
        Self { all, __: PhantomData }
    }
}

impl<X, T: Copy, const N: usize> Table<X, T, N> {
    pub const fn filled(value: T) -> Self {
        Self { all: [value; N], __: PhantomData }
    }
}

impl<X, T: Copy + Empty, const N: usize> Empty for Table<X, T, N> {
    const EMPTY: Self = Self::filled(T::EMPTY);
}

impl<X, T: Copy + Empty, const N: usize> Table<X, T, N> {
    pub const fn empty() -> Self {
        Self::filled(T::EMPTY)
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
/// Table-only form:
///
/// ```ignore
/// finite_set!(
///     /// Board square.
///     Square,
///     /// Array-backed table keyed by square.
///     SquareTable {
///         /// Lower-left square from White's perspective.
///         A1 = 0,
///         B1,
///     }
/// );
/// ```
///
/// Attributes are optional at every documented position:
///
/// ```ignore
/// finite_set!(Square, SquareTable {
///     A1 = 0,
///     B1,
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
///     Players {
///         /// Black player.
///         Black = 0 as black,
///         /// White player.
///         White = 1 as white,
///     }
/// );
/// ```
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
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)?),+ $(,)?
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
                $($(#[$variant_meta])* $variant $(= $value)?),+
            }
        }
    };

    (
        $(#[$name_meta:meta])*
        $name:ident,
        $(#[$record_meta:meta])*
        $record:ident,
        $(#[$table_meta:meta])*
        $table:ident,
        $record_expr:ident {
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
            $record_expr {
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
            $($(#[$variant_meta:meta])* $variant:ident $(= $value:tt)?),+ $(,)?
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

        impl $name {
            pub const LEN: usize = $len;

            pub const ALL: [Self; $len] = <Self as $crate::finite::FiniteSet<$len>>::ALL;

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

            #[track_caller]
            pub const fn from_index(index: u8) -> Option<Self> {
                if index < Self::LEN as u8 {
                    Some(Self::panicky_from_index(index))
                } else {
                    None
                }
            }

            #[track_caller]
            pub(crate) const fn panicky_from_index(index: u8) -> Self {
                assert!(index < Self::LEN as u8);
                Self::ALL[index as usize]
            }

            pub fn iter() -> impl Iterator<Item = Self> {
                <Self as $crate::finite::FiniteSet<$len>>::iter()
            }

            pub fn iter_rev() -> impl Iterator<Item = Self> {
                <Self as $crate::finite::FiniteSet<$len>>::iter_rev()
            }
        }

        impl<T: Copy> $crate::finite::Table<$name, T, $len> {
            pub const fn get(&self, key: $name) -> T {
                self.all[key.index()]
            }
        }

        impl<T> $crate::finite::Table<$name, T, $len> {
            pub const fn get_ref(&self, key: $name) -> &T {
                &self.all[key.index()]
            }

            pub const fn get_mut(&mut self, key: $name) -> &mut T {
                &mut self.all[key.index()]
            }
        }

        impl<T> core::ops::Index<$name> for $crate::finite::Table<$name, T, $len> {
            type Output = T;

            fn index(&self, key: $name) -> &T {
                &self.all[key.index()]
            }
        }

        impl<T> core::ops::IndexMut<$name> for $crate::finite::Table<$name, T, $len> {
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
        $record_expr:ident {
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
            $record_expr {
                $($(#[$variant_meta])* $variant $(= $value)?),+
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
            pub const fn get(&self, key: $name) -> T {
                *self.get_ref(key)
            }
        }

        impl<T> $record<T> {
            pub const fn get_ref(&self, key: $name) -> &T {
                match key {
                    $( $name::$variant => &self.$field ),+
                }
            }

            pub const fn get_mut(&mut self, key: $name) -> &mut T {
                match key {
                    $( $name::$variant => &mut self.$field ),+
                }
            }
        }

        impl<T> core::ops::Index<$name> for $record<T> {
            type Output = T;

            fn index(&self, key: $name) -> &T {
                self.get_ref(key)
            }
        }

        impl<T> core::ops::IndexMut<$name> for $record<T> {
            fn index_mut(&mut self, key: $name) -> &mut T {
                self.get_mut(key)
            }
        }
    };

    (@count $($variant:ident),+) => {
        <[()]>::len(&[$($crate::finite_set!(@unit $variant)),+])
    };

    (@unit $variant:ident) => {
        ()
    };
}
