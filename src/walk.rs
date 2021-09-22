// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;

use rkyv::Archive;

use crate::branch::{Branch, Level, LevelNode};
use crate::branch_mut::{BranchMut, LevelMut};
use crate::compound::{ArchivedChildren, Child, Compound, MutableLeaves};
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
    C::Archived: ArchivedChildren<C, A>,
    A: Annotation<C::Leaf>,
{
    /// Walk over an immutable tree
    Level(&'a Level<'a, C, A>),
    /// Walk over a mutable tree
    LevelMut(&'a LevelMut<'a, C, A>),
}

impl<'a, C, A> Walk<'a, C, A>
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    A: Annotation<C::Leaf>,
{
    pub(crate) fn new(level: &'a Level<C, A>) -> Self {
        Walk::Level(level)
    }

    pub(crate) fn new_mut(level: &'a LevelMut<C, A>) -> Self {
        Walk::LevelMut(level)
    }

    /// Returns the child at specific offset relative to the level offset
    pub fn with_child<F>(&self, ofs: usize, mut f: F) -> Option<Step>
    where
        C: Compound<A>,
        <C::Leaf as Archive>::Archived: Borrow<C::Leaf>,
        A: Annotation<C::Leaf>,
        F: FnMut(WalkChild<C::Leaf, A>) -> Option<Step>,
    {
        match self {
            Walk::Level(level) => match &level.node {
                LevelNode::Root(_r) => todo!(),
                LevelNode::Val(_v) => todo!(),
                LevelNode::Archived(_a) => todo!(),
            },
            Walk::LevelMut(level) => match level.child(ofs) {
                Child::Leaf(t) => f(WalkChild::Leaf(t)),
                Child::Node(n) => f(WalkChild::Annotation(&n.annotation())),
                Child::Empty => f(WalkChild::Empty),
                Child::EndOfNode => f(WalkChild::EndOfNode),
            },
        }
    }
}

/// The trait used to construct a `Branch` or to iterate through a tree.
pub trait Walker<C, A>
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    A: Annotation<C::Leaf>,
{
    /// Walk the tree node, returning the appropriate `Step`
    fn walk(&mut self, walk: Walk<C, A>) -> Step;
}

pub enum WalkChild<'a, T, A> {
    Leaf(&'a T),
    Annotation(&'a A),
    Empty,
    EndOfNode,
}

/// Walker that visits all leaves
pub struct AllLeaves;

impl<C, A> Walker<C, A> for AllLeaves
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    <C::Leaf as Archive>::Archived: Borrow<C::Leaf>,
    A: Annotation<C::Leaf>,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step {
        for i in 0.. {
            if let Some(step) = walk.with_child(i, |child| match child {
                WalkChild::Leaf(_) => Some(Step::Found(i)),
                WalkChild::Annotation(_) => Some(Step::Into(i)),
                WalkChild::Empty => None,
                WalkChild::EndOfNode => Some(Step::Advance),
            }) {
                return step;
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
    Self::Archived: ArchivedChildren<Self, A>,
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
    C::Archived: ArchivedChildren<C, A>,
    <C::Leaf as Archive>::Archived: Borrow<C::Leaf>,
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
