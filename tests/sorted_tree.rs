// use rkyv::ser::Serializer;
// use rkyv::{Archive, Serialize};
// use std::cmp::Ordering;

// use microkelvin::Link;

// impl<T, A> Default for NaiveTree<T, A> {
//     fn default() -> Self {
//         NaiveTree::Empty
//     }
// }

// #[derive(Clone, Archive, Serialize)]
// #[archive(bound(archive = "
//   T: Archive,
//   A: Archive"))]
// #[archive(bound(serialize = "
//   T: 'static + Archive + Chonkable,
//   A: 'static + Clone + Archive + Chonkable,
//   __S: Serializer + Sized"))]
// enum NaiveTree<T, A> {
//     Empty,
//     Single(T),
//     Double(T, T),
//     Middle(
//         #[omit_bounds] Link<NaiveTree<T, A>, A>,
//         T,
//         #[omit_bounds] Link<NaiveTree<T, A>, A>,
//     ),
// }

// impl<T, A> NaiveTree<T, A>
// where
//     T: 'static + Archive<Archived = T> + Ord + Clone,
//     A: 'static + Archive<Archived = A> + Annotation<NaiveTree<T, A>> + Clone,
// {
//     fn new() -> Self {
//         Default::default()
//     }

//     fn insert(&mut self, t: T) {
//         match std::mem::take(self) {
//             NaiveTree::Empty => *self = NaiveTree::Single(t),

//             NaiveTree::Single(a) => {
//                 *self = match t.cmp(&a) {
//                     Ordering::Less => NaiveTree::Double(t, a),
//                     Ordering::Equal => NaiveTree::Single(a),
//                     Ordering::Greater => NaiveTree::Double(a, t),
//                 }
//             }
//             NaiveTree::Double(a, b) => {
//                 *self = match (t.cmp(&a), t.cmp(&b)) {
//                     (Ordering::Equal, _) | (_, Ordering::Equal) => {
//                         NaiveTree::Double(a, b)
//                     }
//                     (Ordering::Greater, Ordering::Greater) => {
//                         NaiveTree::Middle(
//                             Link::new(NaiveTree::Single(a)),
//                             b,
//                             Link::new(NaiveTree::Single(t)),
//                         )
//                     }
//                     (Ordering::Less, Ordering::Less) => NaiveTree::Middle(
//                         Link::new(NaiveTree::Single(t)),
//                         a,
//                         Link::new(NaiveTree::Single(b)),
//                     ),
//                     (Ordering::Greater, Ordering::Less) => NaiveTree::Middle(
//                         Link::new(NaiveTree::Single(a)),
//                         t,
//                         Link::new(NaiveTree::Single(b)),
//                     ),
//                     _ => unreachable!(),
//                 }
//             }
//             NaiveTree::Middle(mut left, mid, mut right) => {
//                 *self = match t.cmp(&mid) {
//                     Ordering::Less => {
//                         left.insert(t);
//                         NaiveTree::Middle(left, mid, right)
//                     }
//                     Ordering::Equal => NaiveTree::Middle(left, mid, right),
//                     Ordering::Greater => {
//                         right.insert(t);
//                         NaiveTree::Middle(left, mid, right)
//                     }
//                 }
//             }
//         }
//     }

//     fn member(&self, t: &T) -> bool {
//         match self {
//             NaiveTree::Empty => false,
//             NaiveTree::Single(a) => a == t,
//             NaiveTree::Double(a, b) => a == t || b == t,
//             NaiveTree::Middle(left, mid, right) => match t.cmp(&mid) {
//                 Ordering::Less => left.member(t),
//                 Ordering::Equal => true,
//                 Ordering::Greater => right.member(t),
//             },
//         }
//     }
// }

// #[cfg(test)]
// mod test {
//     use super::*;

//     use std::io;
//     use tempfile::tempdir;

//     use rand::Rng;
//     use rend::LittleEndian;

//     #[test]
//     fn many_many_many() -> Result<(), io::Error> {
//         const N: u16 = 3;

//         let mut rng = rand::thread_rng();
//         let mut numbers: Vec<u16> = vec![];

//         for _ in 0..N {
//             numbers.push(rng.gen());
//         }

//         let mut tree = NaiveTree::<LittleEndian<u16>, ()>::new();

//         for n in &numbers {
//             let n: LittleEndian<_> = (*n).into();
//             tree.insert(n);
//         }

//         for n in &numbers {
//             let n: LittleEndian<_> = (*n).into();
//             assert_eq!(tree.member(&n), true)
//         }

//         // let mut serializer = AllocSerializer::<4096>::default();
//         // let _ofs = serializer.serialize_value(&tree)?;

//         // let buf = serializer.into_serializer().into_inner();
//         // let archived_tree = unsafe {
//         // archived_root::<NaiveTree<LittleEndian<u16>, ()>>(&buf) };

//         // let dir = tempdir()?;
//         // Portal::initialize(dir.path());

//         let ofs = Portal::put(&tree);

//         let archived_tree = Portal::get(ofs);

//         for n in &numbers {
//             let n: LittleEndian<_> = (*n).into();
//             assert_eq!(archived_tree.member(&n), true)
//         }

//         Ok(())
//     }
// }
