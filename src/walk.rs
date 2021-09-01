// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;

use crate::backend::Getable;
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{Child, Compound, MutableLeaves};
use crate::error::Error;

/// The return value from a closure to `walk` the tree.
///
/// Determines how the `Branch` is constructed
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

/// The argument given to a `Walker` to traverse through nodes.
pub struct Walk<'a, C, A> {
    ofs: usize,
    compound: &'a C,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Walk<'a, C, A> {
    pub(crate) fn new(compound: &'a C, ofs: usize) -> Self {
        Walk {
            ofs,
            compound,
            _marker: PhantomData,
        }
    }

    /// Returns the child at specific offset relative to the branch offset
    pub fn child(&self, ofs: usize) -> Child<'a, C, A>
    where
        C: Compound<A>,
    {
        self.compound.child(ofs + self.ofs)
    }
}

/// The trait used to construct a `Branch` or to iterate through a tree.
pub trait Walker<C, A> {
    /// Walk the tree node, returning the appropriate `Step`
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

/// Trait that provides a `first` and `first_mut` method to any Compound with a
/// Cardinality annotation
pub trait First<'a, A>
where
    Self: Compound<A>,
{
    /// Construct a `Branch` pointing to the first element, if not empty
    fn first(&'a self) -> Result<Option<Branch<'a, Self, A>>, Error>;

    /// Construct a `BranchMut` pointing to the first element, if not empty
    fn first_mut(&'a mut self) -> Result<Option<BranchMut<'a, Self, A>>, Error>
    where
        Self: MutableLeaves + Clone;
}

impl<'a, C, A> First<'a, A> for C
where
    C: Compound<A> + Getable,
{
    fn first(&'a self) -> Result<Option<Branch<'a, Self, A>>, Error> {
        Branch::<_, A>::walk(self, AllLeaves)
    }

    fn first_mut(&'a mut self) -> Result<Option<BranchMut<'a, Self, A>>, Error>
    where
        C: MutableLeaves + Clone,
    {
        BranchMut::<_, A>::walk(self, AllLeaves)
    }
}
