// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;

use canonical::CanonError;

use crate::annotations::Combine;
use crate::branch::Branch;
use crate::branch_mut::BranchMut;
use crate::compound::{Child, Compound, MutableLeaves};

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

impl<'a, C, A> Walk<'a, C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    pub(crate) fn new(compound: &'a C, ofs: usize) -> Self {
        Walk {
            ofs,
            compound,
            _marker: PhantomData,
        }
    }

    /// Returns the child at specific offset relative to the branch offset
    pub fn child(&self, ofs: usize) -> Child<'a, C, A> {
        self.compound.child(ofs + self.ofs)
    }
}

pub trait Walker<C, A>
where
    C: Compound<A>,
    A: Combine<C, A>,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step;
}

/// Walker that visits all leaves
pub struct AllLeaves;

impl<C, A> Walker<C, A> for AllLeaves
where
    C: Compound<A>,
    A: Combine<C, A>,
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
    A: Combine<Self, A>,
{
    /// Construct a `Branch` pointing to the first element, if not empty
    fn first(&'a self) -> Result<Option<Branch<'a, Self, A>>, CanonError>;

    /// Construct a `BranchMut` pointing to the first element, if not empty
    fn first_mut(
        &'a mut self,
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError>
    where
        Self: MutableLeaves;
}

impl<'a, C, A> First<'a, A> for C
where
    C: Compound<A> + Clone,
    A: Combine<C, A>,
{
    fn first(&'a self) -> Result<Option<Branch<'a, Self, A>>, CanonError> {
        Branch::<_, A>::walk(self, AllLeaves)
    }

    fn first_mut(
        &'a mut self,
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError>
    where
        A: Combine<Self, A>,
        C: MutableLeaves,
    {
        BranchMut::<_, A>::walk(self, AllLeaves)
    }
}
