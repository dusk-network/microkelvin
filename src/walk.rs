// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::annotations::{ARef, Annotation};
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{ArchivedCompound, Compound, MutableLeaves};
use crate::primitive::Primitive;

/// The return value from a closure to `walk` the tree.
///
/// Determines how the `Branch` is constructed
#[derive(Debug)]
pub enum Step {
    /// The correct leaf was found!
    Found(usize),
    /// Advance search
    Advance,
    /// Abort search
    Abort,
}

/// The trait used to construct a `Branch` or to iterate through a tree.
pub trait Walker<C, A>
where
    C: Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Primitive + Annotation<C::Leaf>,
{
    /// Walk the tree node, returning the appropriate `Step`
    fn walk(&mut self, walk: impl Slots<C, A>) -> Step;
}

#[derive(Debug)]
pub enum Slot<'a, C, A>
where
    C: Compound<A>,
    A: Primitive + Annotation<C::Leaf>,
{
    Leaf(&'a C::Leaf),
    Annotation(ARef<'a, A>),
    Empty,
    End,
}

pub trait Slots<C, A>
where
    C: Compound<A>,
    A: Primitive + Annotation<C::Leaf>,
{
    fn slot(&self, ofs: usize) -> Slot<C, A>;
}

/// Walker that visits all leaves
#[derive(Debug)]
pub struct AllLeaves;

impl<C, A> Walker<C, A> for AllLeaves
where
    C: Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Primitive + Annotation<C::Leaf>,
{
    fn walk(&mut self, walk: impl Slots<C, A>) -> Step {
        for i in 0.. {
            match walk.slot(i) {
                Slot::End => return Step::Abort,
                Slot::Empty => (),
                _ => return Step::Found(i),
            }
        }
        unreachable!()
    }
}

/// Trait that provides a `first` and `first_mut` method to any Compound with a
/// Cardinality annotation
pub trait First<'a, A>
where
    Self: Compound<A>,
    Self::Archived: ArchivedCompound<Self, A>,
    A: Primitive + Annotation<Self::Leaf>,
{
    /// Construct a `Branch` pointing to the first element, if not empty
    fn first(&'a self) -> Option<Branch<'a, Self, A>>;

    /// Construct a `BranchMut` pointing to the first element, if not empty
    fn first_mut(&'a mut self) -> Option<BranchMut<'a, Self, A>>
    where
        Self: MutableLeaves + Clone;
}

impl<'a, C, A> First<'a, A> for C
where
    C: Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Primitive + Annotation<C::Leaf>,
{
    fn first(&'a self) -> Option<Branch<'a, Self, A>> {
        Branch::<_, A>::walk(self, AllLeaves)
    }

    fn first_mut(&'a mut self) -> Option<BranchMut<'a, Self, A>>
    where
        C: MutableLeaves + Clone,
    {
        BranchMut::<_, A>::walk(self, AllLeaves)
    }
}
