/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements of a collection
use crate::branch::{Branch, Step, Walk};
use crate::branch_mut::{BranchMut, StepMut, WalkMut};
use crate::Compound;
use canonical::{Canon, CanonError};
use canonical_derive::Canon;
use core::borrow::Borrow;

/// The cardinality of a compound collection
#[derive(Canon, PartialEq, Debug, Clone)]
pub struct Cardinality(pub(crate) u64);

impl Into<u64> for &Cardinality {
    fn into(self) -> u64 {
        self.0
    }
}

// impl<C> Annotation<C> for Cardinality
// where
//     C: Compound<A>,
// {
//     fn identity() -> Self {
//         Cardinality(0)
//     }

//     fn from_leaf(_: &C::Leaf) -> Self {
//         Cardinality(1)
//     }

//     fn from_node(node: &C) -> Self {
//         let mut c = 0;
//         for i in 0.. {
//             c += match node.child::<Self>(i) {
//                 Child::Leaf(_) => 1,
//                 Child::Node(n) => n.annotation().borrow().0,
//                 Child::EndOfNode => return Cardinality(c),
//                 Child::Empty => 0,
//             }
//         }
//         unreachable!()
//     }
// }

/// Find the nth element of any collection satisfying the given annotation
/// constraints
pub trait Nth<'a, A>
where
    Self: Compound<A>,
    A: Borrow<Cardinality>,
{
    /// Construct a `Branch` pointing to the `nth` element, if any
    fn nth(&'a self, n: u64)
        -> Result<Option<Branch<'a, Self, A>>, CanonError>;

    /// Construct a `BranchMut` pointing to the `nth` element, if any
    fn nth_mut(
        &'a mut self,
        n: u64,
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError>;
}

impl<'a, C, A> Nth<'a, A> for C
where
    C: Compound<A>,
    A: Borrow<Cardinality>,
{
    fn nth(
        &'a self,
        mut index: u64,
    ) -> Result<Option<Branch<'a, Self, A>>, CanonError> {
        Branch::<_, A>::walk(self, |f| match f {
            Walk::Leaf(l) => {
                if index == 0 {
                    Step::Found(l)
                } else {
                    index -= 1;
                    Step::Next
                }
            }
            Walk::Node(n) => {
                let &Cardinality(card) = n.annotation().borrow();
                if card <= index {
                    index -= card;
                    Step::Next
                } else {
                    Step::Into(n)
                }
            }
            Walk::Abort => Step::Abort,
        })
    }

    fn nth_mut(
        &'a mut self,
        mut index: u64,
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError> {
        BranchMut::<_, A>::walk(self, |f| match f {
            WalkMut::Leaf(l) => {
                if index == 0 {
                    StepMut::Found(l)
                } else {
                    index -= 1;
                    StepMut::Next
                }
            }
            WalkMut::Node(n) => {
                let &Cardinality(card) = n.annotation().borrow();
                if card <= index {
                    index -= card;
                    StepMut::Next
                } else {
                    StepMut::Into(n)
                }
            }
        })
    }
}
