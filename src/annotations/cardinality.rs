// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

/// Annotation to keep track of the cardinality,
/// i.e. the amount of elements in a collection
use core::borrow::Borrow;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use crate::annotations::{Annotation, Combine};
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{AnnoIter, ArchivedChildren, Compound, MutableLeaves};
use crate::walk::{Step, Walk, WalkChild, Walker};

/// The cardinality of a compound collection
#[derive(
    PartialEq, Debug, Clone, Default, Copy, Archive, Serialize, Deserialize,
)]
#[archive_attr(derive(CheckBytes))]
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
    A: Borrow<Self>,
{
    fn combine<C>(iter: AnnoIter<C, A>) -> Self
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        Cardinality(iter.fold(0, |sum, ann| sum + (*ann).borrow().0))
    }
}

/// Walker method to find the nth element of a compound collection
pub struct Offset(u64);

impl<C, A> Walker<C, A> for Offset
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    A: Annotation<C::Leaf> + Borrow<Cardinality> + Archive,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step {
        for i in 0.. {
            walk.with_child(i, |child| match child {
                WalkChild::Leaf(_) => {
                    if self.0 == 0 {
                        Some(Step::Found(i))
                    } else {
                        self.0 -= 1;
                        None
                    }
                }
                WalkChild::Annotation(a) => {
                    let card: u64 = a.borrow().into();
                    if card <= self.0 {
                        self.0 -= card;
                        None
                    } else {
                        Some(Step::Into(i))
                    }
                }
                WalkChild::Empty => None,
                WalkChild::EndOfNode => Some(Step::Abort),
            });
        }
        unreachable!()
    }
}

/// Trait that provides `nth()` and `nth_mut()` methods to any Compound with a
/// Cardinality annotation
pub trait Nth<'a, A>
where
    Self: Compound<A>,
    Self::Leaf: Archive,
    Self::Archived: ArchivedChildren<Self, A>,
    A: Annotation<Self::Leaf>,
{
    /// Construct a `Branch` pointing to the `nth` element, if any
    fn nth(&'a self, n: u64) -> Option<Branch<'a, Self, A>>;

    /// Construct a `BranchMut` pointing to the `nth` element, if any
    fn nth_mut(&'a mut self, n: u64) -> Option<BranchMut<'a, Self, A>>
    where
        Self: MutableLeaves + Clone;
}

impl<'a, C, A> Nth<'a, A> for C
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    A: Annotation<C::Leaf> + Borrow<Cardinality>,
{
    fn nth(&'a self, ofs: u64) -> Option<Branch<'a, Self, A>> {
        // Return the first that satisfies the walk
        Branch::<_, A>::walk(self, Offset(ofs))
    }

    fn nth_mut(&'a mut self, ofs: u64) -> Option<BranchMut<'a, Self, A>>
    where
        C: MutableLeaves + Clone,
    {
        // Return the first mutable branch that satisfies the walk
        BranchMut::<_, A>::walk(self, Offset(ofs))
    }
}
