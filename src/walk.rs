// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::branch::{Branch, Level};
use crate::branch_mut::{BranchMut, LevelMut};
use crate::compound::{Compound, MutableLeaves};
use crate::Annotation;

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

/// Type to handle the walking over datastructures
pub enum Walk<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Walk over an immutable tree
    Level(&'a Level<'a, C, A>),
    /// Walk over a mutable tree
    LevelMut(&'a LevelMut<'a, C, A>),
}

// /// The argument given to a `Walker` to traverse through nodes.
// pub struct Walk<'a, C, A> {
//     compound: &'a C,
//     _marker: PhantomData<A>,
// }

impl<'a, C, A> Walk<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    pub(crate) fn new(level: &'a Level<C, A>) -> Self {
        Walk::Level(level)
    }

    pub(crate) fn new_mut(level: &'a LevelMut<C, A>) -> Self {
        Walk::LevelMut(level)
    }

    /// Returns the child at specific offset relative to the level offset
    pub fn child(&self, _ofs: usize) -> WalkChild<'a, C::Leaf, A>
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        match self {
            Walk::Level(_) => todo!(),
            Walk::LevelMut(_) => todo!(),
        }
    }
}

/// The trait used to construct a `Branch` or to iterate through a tree.
pub trait Walker<C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Walk the tree node, returning the appropriate `Step`
    fn walk(&mut self, walk: Walk<C, A>) -> Step;
}

pub enum WalkChild<'a, T, A> {
    Leaf(T),
    Annotation(&'a A),
    Empty,
    EndOfNode,
}

/// Walker that visits all leaves
pub struct AllLeaves;

impl<C, A> Walker<C, A> for AllLeaves
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step {
        for i in 0.. {
            match walk.child(i) {
                WalkChild::Leaf(_) => return Step::Found(i),
                WalkChild::Annotation(_) => return Step::Into(i),
                WalkChild::Empty => (),
                WalkChild::EndOfNode => return Step::Advance,
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
    A: Annotation<Self::Leaf>,
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
    A: Annotation<C::Leaf>,
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
