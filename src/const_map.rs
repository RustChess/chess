use core::{marker::PhantomData, ops};
use std::collections::BTreeMap;

use crate::position::{All, Square};

#[cfg(feature = "serde")]
use serde::Serialize;

/// "Full" map, containing an element of T for any element of X
///
/// They are accessed by indexing, e.g. `let t = map[x]` and `map[x] = t`
///
/// Note that `[T; N]` does not always implement Default, e.g. for `N = 64`.
/// For this reason, we define ad-hoc inherent methods `fn default64()` for
/// all `T: Default` on all `Map<T, X, 64>` etc. Note that we can implement
/// functions named `fn default()`, but the compiler will get confused by
/// use of `Map::default()` for those N where usual Default *is* defined...
#[derive(Clone, Copy, Debug)]
pub struct Map<X, T, const N: usize> {
    all: [T; N],
    __: PhantomData<X>,
}

impl<X, T, const N: usize> From<Map<X, T, N>> for BTreeMap<X, T>
where
    T: Copy,
    X: All<N> + Ord,
{
    fn from(map: Map<X, T, N>) -> Self {
        let mut bmap = BTreeMap::new();
        for x in 0..N {
            bmap.insert(X::ALL[x], map.all[x]);
        }
        bmap
    }
}

// #[cfg(feature = "serde")]
// impl<'de, X, T: Deserialize<'de>, const N: usize> Deserialize<'de> for Map<X, T, N> {
//     fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
//     where
//         D: serde::Deserializer<'de>,
//     {
//         let all: Vec<T> = Deserialize::deserialize(deserializer)?;
//         let all: [T; N] = all.try_into().map_err(|_| serde::de::Error::custom("wrong length"))?;
//         Ok(Self { all, __: PhantomData })
//     }
// }

#[cfg(feature = "serde")]
impl<X, T: Serialize, const N: usize> Serialize for Map<X, T, N>
where
    T: Copy + Serialize,
    X: All<N> + Ord + Serialize,
{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        Serialize::serialize(&BTreeMap::from(*self), serializer)
    }
}

impl<X, T: Default, const N: usize> Default for Map<X, T, N>
where
    [T; N]: Default,
{
    fn default() -> Self {
        Self { all: Default::default(), __: PhantomData }
    }
}

impl<X, T: Copy + Default> Map<X, T, 64> {
    /// Need this because [T; 64] does not implement Default for historical reasons
    pub fn default64() -> Self {
        Self { all: [T::default(); 64], __: PhantomData }
    }
}

impl<X: All<N>, T, const N: usize> ops::Index<X> for Map<X, T, N> {
    type Output = T;
    fn index(&self, value: X) -> &T {
        &self.all[value.index()]
    }
}

impl<X: All<N>, T, const N: usize> ops::IndexMut<X> for Map<X, T, N> {
    fn index_mut(&mut self, value: X) -> &mut T {
        &mut self.all[value.index()]
    }
}

macro_rules! const_map {
    ($key:ty, $len:expr) => {
        // impl<V> Map<$key, V, $len> {
        //     // pub const fn new(values: [V; $len]) -> Self {
        //     //     Self { values, _key: core::marker::PhantomData }
        //     // }

        //     pub const fn get(&self, key: $key) -> &V {
        //         &self.all[key.index_const()]
        //     }
        // }

        impl<V: Copy> Map<$key, V, $len> {
            pub const fn get(&self, key: $key) -> V {
                self.all[key.index_const()]
            }
        }
    };
}

const_map!(Square, 64);

// macro_rules! const_map_from_fn {
//     ($key:ty, |$x:ident| $value:expr) => {{
//         let mut all = vec![$key::EMPTY; $len];
//         let mut i = 0;
//         while i < $len {
//             let $x = <$key>::ALL[i];
//             all[i] = $value;
//             i += 1;
//         }
//         Map::<$key, _, $len>::new(all)
//     }};
// }
