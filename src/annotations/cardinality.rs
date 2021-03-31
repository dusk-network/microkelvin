// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements in a collection
use canonical::CanonError;
use canonical_derive::Canon;
use core::borrow::Borrow;

use crate::annotations::{Ann, Annotation};
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{Compound, MutableLeaves};
use crate::walk::{Step, Walk, Walker};

/// The cardinality of a compound collection
#[derive(Canon, PartialEq, Debug, Clone, Default)]
pub struct Cardinality(pub(crate) u64);

impl From<Cardinality> for u64 {
    fn from(c: Cardinality) -> Self {
        c.0
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

/// Walker method to find the nth element of a compound collection
pub struct Offset(u64);

impl<C, A> Walker<C, A> for Offset
where
    C: Compound<A>,
    A: Annotation<C::Leaf> + Borrow<Cardinality>,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step {
        match walk {
            Walk::Leaf(_) => {
                // Walk found a leaf!
                if self.0 == 0 {
                    // if we're already at our destination, we're done!
                    Step::Found
                } else {
                    // else, we subtract one and try again
                    self.0 -= 1;
                    Step::Next
                }
            }
            Walk::Ann(ann) => {
                // Walk found an annotated subtree, let's borrow it's annotation
                // as `Cardinality` as per the generic bounds on
                // `A`
                let &Cardinality(card) = ann.borrow();

                if card <= self.0 {
                    // The subtree is smaller than our remainder, subtract and
                    // continue
                    self.0 -= card;
                    Step::Next
                } else {
                    // The subtree is larger than our remainder, descend into
                    // it
                    Step::Into
                }
            }
        }
    }
}

/// Trait that provides a nth() method to any Compound with a Cardinality
/// annotation
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
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError>
    where
        Self: MutableLeaves;
}

impl<'a, C, A> Nth<'a, A> for C
where
    C: Compound<A> + Clone,
    A: Annotation<Self::Leaf> + Borrow<Cardinality>,
{
    fn nth(
        &'a self,
        ofs: u64,
    ) -> Result<Option<Branch<'a, Self, A>>, CanonError> {
        // Return the first that satisfies the walk
        Branch::<_, A>::walk(self, Offset(ofs))
    }

    fn nth_mut(
        &'a mut self,
        ofs: u64,
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError>
    where
        C: MutableLeaves,
    {
        // Return the first mutable branch that satisfies the walk
        BranchMut::<_, A>::walk(self, Offset(ofs))
    }
}
