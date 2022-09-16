// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{Child, Compound, MutableLeaves};

use core::marker::PhantomData;

use ranno::Annotation;

/// The return value from a closure to [`walk`] the tree.
///
/// Determines how the [`Branch`] is constructed
///
/// [`walk`]: Walker::walk
pub enum Step {
    /// The correct leaf was found!
    Found(usize),
    /// Traverse the branch deeper
    Into(usize),
    /// Advance search
    Advance,
    /// Abort search
    Abort,
}

/// The argument given to a [`Walker`] to traverse through nodes.
pub struct Walk<'a, C, A> {
    index: usize,
    compound: &'a C,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Walk<'a, C, A> {
    pub(crate) fn new(compound: &'a C, index: usize) -> Self {
        Walk {
            index,
            compound,
            _marker: PhantomData,
        }
    }
}

impl<'a, C, A> Walk<'a, C, A>
where
    C: Compound<A>,
{
    /// Returns the child at specific index relative to the branch index
    pub fn child(&self, index: usize) -> Child<'a, C, A> {
        self.compound.child(index + self.index)
    }
}

/// The trait used to construct a [`Branch`] or to iterate through a tree.
pub trait Walker<C, A> {
    /// Walk the tree node, returning the appropriate [`Step`]
    fn walk(&mut self, walk: Walk<C, A>) -> Step;
}

/// Walker that visits all leaves
pub struct AllLeaves;

impl<C, A> Walker<C, A> for AllLeaves
where
    C: Compound<A>,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step {
        for i in 0.. {
            match walk.child(i) {
                Child::Leaf(_) => return Step::Found(i),
                Child::Node(_) => return Step::Into(i),
                Child::Empty => (),
                Child::EndOfNode => return Step::Advance,
            }
        }
        unreachable!()
    }
}

/// Trait that provides a [`first`] and [`first_mut`] method to any
/// [`Compound`].
///
/// [`first`]: First::first
/// [`first_mut`]: First::first_mut
pub trait First<A>: Sized + Compound<A> {
    /// Construct a [`Branch`] pointing to the first element, if not empty
    fn first(&self) -> Option<Branch<Self, A>>;

    /// Construct a [`BranchMut`] pointing to the first element, if not empty
    fn first_mut(&mut self) -> Option<BranchMut<Self, A>>
    where
        Self: MutableLeaves;
}

impl<C, A> First<A> for C
where
    C: Compound<A>,
    A: Annotation<C>,
{
    fn first(&self) -> Option<Branch<Self, A>> {
        Branch::walk(self, AllLeaves)
    }

    fn first_mut(&mut self) -> Option<BranchMut<Self, A>>
    where
        C: MutableLeaves,
    {
        BranchMut::walk(self, AllLeaves)
    }
}
