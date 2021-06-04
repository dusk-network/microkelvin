// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements in a collection
use core::borrow::Borrow;

use canonical::{Canon, CanonError};
use canonical_derive::Canon;

use crate::annotations::{Annotation, Combine};
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{AnnoIter, Child, Compound, MutableLeaves};
use crate::walk::{Step, Walk, Walker};

/// The cardinality of a compound collection
#[derive(Canon, PartialEq, Debug, Clone, Default, Copy)]
pub struct Cardinality(pub(crate) u64);

impl From<Cardinality> for u64 {
    fn from(c: Cardinality) -> Self {
        c.0
    }
}

impl<'a> From<&'a Cardinality> for u64 {
    fn from(c: &'a Cardinality) -> Self {
        c.0
    }
}

impl<L> Annotation<L> for Cardinality {
    fn from_leaf(_: &L) -> Self {
        Cardinality(1)
    }
}

impl<A> Combine<A> for Cardinality
where
    A: Borrow<Self> + Canon,
{
    fn combine<C>(iter: AnnoIter<C, A>) -> Self
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        let mut sum = 0;
        for ann in iter {
            let card = (*ann).borrow();
            sum += card.0
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
        for i in 0.. {
            match walk.child(i) {
                Child::Leaf(_) => {
                    if self.0 == 0 {
                        return Step::Found(i);
                    } else {
                        self.0 -= 1
                    }
                }
                Child::Node(node) => {
                    let card: u64 = (*node.annotation()).borrow().into();

                    if card <= self.0 {
                        self.0 -= card;
                    } else {
                        return Step::Into(i);
                    }
                }
                Child::Empty => (),
                Child::EndOfNode => return Step::Abort,
            }
        }
        unreachable!()
    }
}

/// Trait that provides `nth()` and `nth_mut()` methods to any Compound with a
/// Cardinality annotation
pub trait Nth<'a, A>
where
    Self: Compound<A>,
    A: Annotation<Self::Leaf> + Borrow<Cardinality>,
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
    A: Annotation<C::Leaf> + Borrow<Cardinality>,
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
