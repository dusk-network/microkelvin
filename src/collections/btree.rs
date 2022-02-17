// use core::borrow::Borrow;
// use core::cmp::Ordering;
// use core::mem;

// use bytecheck::CheckBytes;
// use rkyv::{Archive, Deserialize, Serialize};

// use crate::tower::{Fundamental, WellArchived, WellFormed};
// use crate::{Annotation, Child, ChildMut, Compound, Link, MaxKey};

// /// A BTree key-value pair
// #[derive(Archive, Clone, Serialize, Deserialize)]
// #[archive_attr(derive(CheckBytes))]
// pub struct Pair<K, V> {
//     /// The key of the key-value pair
//     pub key: K,
//     /// The value of the key-value pair
//     pub val: V,
// }

// #[derive(Archive, Clone, Serialize, Deserialize)]
// #[archive_attr(derive(CheckBytes))]
// struct Leaves<K, V, const LE: usize>(Vec<Pair<K, V>>);

// #[derive(Archive, Clone, Serialize, Deserialize)]
// #[archive_attr(derive(CheckBytes))]
// struct Links<K, V, A, const LE: usize, const LI: usize>(
//     Vec<Link<BTreeMapInner<K, V, A, LE, LI>, A>>,
// );

// /// A BTreeMap
// pub struct BTreeMap<K, V, A, const LE: usize = 9, const LI: usize = 9>(
//     BTreeMapInner<K, V, A, LE, LI>,
// );

// // We have an inner type to avoid having to make Leaves and Links public
// #[derive(Archive, Clone, Deserialize, Serialize)]
// #[archive_attr(derive(CheckBytes))]
// enum BTreeMapInner<K, V, A, const LE: usize = 9, const LI: usize = 9> {
//     /// A node of leaves
//     Leaves(Leaves<K, V, LE>),
//     /// A node of links
//     Links(Links<K, V, A, LE, LI>),
// }

// impl<K, V, const N: usize> Default for Leaves<K, V, N> {
//     fn default() -> Self {
//         Self(Default::default())
//     }
// }

// impl<K, V, const LE: usize> Leaves<K, V, LE>
// where
//     K: Fundamental + Ord,
// {
//     #[inline(always)]
//     fn split_point() -> usize {
//         (LE + 1) / 2
//     }

//     #[inline(always)]
//     fn full(&self) -> bool {
//         self.0.len() == LE
//     }

//     pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>, Self> {
//         match self
//             .0
//             .binary_search_by(|Pair { key, .. }| -> Ordering { key.cmp(&k) })
//         {
//             Ok(idx) => Ok(Some(mem::replace(&mut self.0[idx].val, v))),
//             Err(idx) => {
//                 if self.full() {
//                     let point = Self::split_point();
//                     let mut rest = self.0.split_off(point);
//                     match rest[0].key.cmp(&k) {
//                         Ordering::Greater => {
//                             self.0.insert(idx, Pair { key: k, val: v });
//                         }
//                         Ordering::Equal => unreachable!(
//                             "Split on equal key instead of replacing"
//                         ),
//                         Ordering::Less => {
//                             rest.insert(idx - point, Pair { key: k, val: v })
//                         }
//                     }
//                     Err(Leaves(rest))
//                 } else {
//                     self.0.insert(idx, Pair { key: k, val: v });
//                     Ok(None)
//                 }
//             }
//         }
//     }

//     pub fn get<O>(&self, k: &O) -> Option<&V>
//     where
//         K: Borrow<O>,
//         O: Ord,
//     {
//         if let Ok(idx) =
//             self.0.binary_search_by(|Pair { key, .. }| -> Ordering {
//                 key.borrow().cmp(k)
//             })
//         {
//             Some(&self.0[idx].val)
//         } else {
//             None
//         }
//     }
// }

// impl<K, V, A, const LE: usize, const LI: usize> Links<K, V, A, LE, LI> {
//     #[inline(always)]
//     const fn split_point() -> usize {
//         (LI + 1) / 2
//     }

//     #[inline(always)]
//     fn full(&self) -> bool {
//         self.0.len() == LI
//     }

//     fn from_leaves(a: Leaves<K, V, LE>, b: Leaves<K, V, LE>) -> Self {
//         let map_a = BTreeMapInner::Leaves(a);
//         let map_b = BTreeMapInner::Leaves(b);
//         let link_a = Link::new(map_a);
//         let link_b = Link::new(map_b);
//         Links(vec![link_a, link_b])
//     }
// }

// impl<K, V, A> Compound<A> for BTreeMapInner<K, V, A>
// where
//     K: Fundamental,
//     K::Archived: WellArchived<K>,
//     V: WellFormed,
//     V::Archived: WellArchived<V>,
//     A: Annotation<Pair<K, V>>,
// {
//     type Leaf = Pair<K, V>;

//     fn child(&self, ofs: usize) -> Child<Self, A> {
//         todo!()
//     }

//     fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A> {
//         todo!()
//     }
// }

// impl<K, V, A> BTreeMapInner<K, V, A>
// where
//     K: Fundamental + Ord,
//     K::Archived: WellArchived<K>,
//     V: WellFormed,
//     V::Archived: WellArchived<V>,
//     A: Fundamental + Annotation<Pair<K, V>> + Borrow<MaxKey<K>>,
//     A::Archived: WellArchived<A>,
// {
//     /// Create a new empty BTreemap
//     pub fn new() -> Self {
//         BTreeMapInner::Leaves(Leaves::default())
//     }

//     /// Insert a key-value pair into the map, returning the replaced value if
//     /// any.
//     pub fn insert(&mut self, k: K, v: V) -> Option<V> {
//         match self {
//             BTreeMapInner::Leaves(pairs) => match pairs.insert(k, v) {
//                 Ok(op) => op,
//                 Err(split) => {
//                     let a = mem::take(pairs);
//                     let b = split;
//                     *self = BTreeMapInner::Links(Links::from_leaves(a, b));
//                     None
//                 }
//             },
//             BTreeMapInner::Links(links) => {
//                 let mut idx = links.0.len() - 1;

//                 for i in 0..links.0.len() {
//                     match (*links.0[i].annotation()).borrow() {
//                         MaxKey::NegativeInfinity => unreachable!(),
//                         MaxKey::Maximum(key) => {
//                             if let Ordering::Greater = key.cmp(&k) {
//                                 idx = i;
//                                 break;
//                             }
//                         }
//                     }
//                 }

//                 // insert in last
//                 let last = links.0[idx];
//                 let mut inner = last.inner_mut();
//                 inner.insert(k, v);
//             }
//         }
//     }

//     /// Insert a key-value pair into the map, returning the replaced value if
//     /// any.
//     pub fn get<O>(&mut self, k: &O) -> Option<&V>
//     where
//         K: Borrow<O>,
//         O: Ord,
//     {
//         match self {
//             BTreeMapInner::Leaves(leaves) => leaves.get(k),
//             _ => todo!(),
//         }
//     }
// }

// impl<K, V, A> BTreeMap<K, V, A>
// where
//     K: Fundamental + Ord,
//     V: WellFormed,
//     A: Annotation<Pair<K, V>> + Borrow<MaxKey<K>>,
// {
//     /// Create a new empty BTreemap
//     pub fn new() -> Self {
//         BTreeMap(BTreeMapInner::new())
//     }

//     /// Insert a key-value pair into the map, returning the replaced value if
//     /// any.
//     pub fn insert(&mut self, k: K, v: V) -> Option<V> {
//         self.0.insert(k, v)
//     }

//     /// Insert a key-value pair into the map, returning the replaced value if
//     /// any.
//     pub fn get<O>(&mut self, k: &O) -> Option<&V>
//     where
//         K: Borrow<O>,
//         O: Ord,
//     {
//         self.0.get(k)
//     }
// }

// #[cfg(test)]
// mod test {
//     use super::BTreeMap;

//     use crate::OffsetLen;

//     use rkyv::rend::LittleEndian;

//     #[test]
//     fn btree_write_read() {
//         let mut map = BTreeMap::<LittleEndian<i32>, i32, (),
// OffsetLen>::new();

//         const N: i32 = 16;

//         for i in 0..N {
//             map.insert(LittleEndian::from(i), i);
//         }

//         for i in 0..N {
//             assert_eq!(map.get(&LittleEndian::from(i)), Some(&i));
//         }
//     }
// }
