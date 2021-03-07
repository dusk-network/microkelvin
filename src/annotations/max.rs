/// Annotation to keep track of the largest element of a collection
// use crate::annotations::Annotation;
// use crate::{Child, Compound};
use canonical::Canon;
use canonical_derive::Canon;
// use core::borrow::Borrow;

/// The maximum value of a collection
#[derive(Canon, PartialEq, Eq, Debug, Clone, Copy)]
pub enum Max<K> {
    /// Identity of max, everything else is larger
    NegativeInfinity,
    /// Actual max value
    Maximum(K),
}

// impl<C, K> Annotation<C> for Max<K>
// where
//     C: Compound,
//     K: Ord + Clone,
//     C::Leaf: Borrow<K>,
// {
//     fn identity() -> Self {
//         Max::NegativeInfinity
//     }

//     fn from_leaf(leaf: &C::Leaf) -> Self {
//         Max::Maximum(leaf.borrow().clone())
//     }

//     fn from_node(node: &C) -> Self {
//         let max = Max::NegativeInfinity;
//         for i in 0.. {
//             match node.child::<Self>(i) {
//                 Child::Leaf(_) => todo!(),
//                 Child::Node(_) => todo!(),
//                 Child::EndOfNode => return max,
//                 Child::Empty => todo!(),
//             }
//         }
//         unreachable!()
//     }
// }
