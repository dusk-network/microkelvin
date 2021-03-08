/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements in a collection
use crate::annotations::{Ann, Annotation};
use crate::branch::{Branch, Step, Walk};
use crate::branch_mut::{BranchMut, StepMut, WalkMut};
use crate::Compound;
use canonical::{Canon, CanonError};
use canonical_derive::Canon;
use core::borrow::Borrow;

/// The cardinality of a compound collection
#[derive(Canon, PartialEq, Debug, Clone, Default)]
pub struct Cardinality(pub(crate) u64);

impl Into<u64> for &Cardinality {
    fn into(self) -> u64 {
        self.0
    }
}

impl<L> Annotation<L> for Cardinality {
    fn from_leaf(_: &L) -> Self {
        Cardinality(1)
    }

    fn combine(annotations: &[Ann<Self>]) -> Self {
        let mut sum = 0;
        for a in annotations {
            sum += a.0
        }
        Cardinality(sum)
    }
}

/// Find the nth element of any collection satisfying the given annotation
/// constraints
pub trait Nth<'a, A>
where
    Self: Compound<A>,
    A: Annotation<Self::Leaf> + Borrow<Cardinality> + Clone,
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
    C: Compound<A> + Clone,
    A: Annotation<Self::Leaf> + Borrow<Cardinality>,
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
