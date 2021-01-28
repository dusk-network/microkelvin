// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;

use canonical::{Canon, Store};

use crate::annotation::{Annotated, Annotation, Cardinality};
use crate::branch::{Branch, Step, Walk};
use crate::branch_mut::{BranchMut, StepMut, WalkMut};

/// The response of the `child` method on a `Compound` node.
pub enum Child<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Child is a leaf
    Leaf(&'a C::Leaf),
    /// Child is an annotated subtree node
    Node(&'a Annotated<C, S>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// The response of the `child_mut` method on a `Compound` node.
pub enum ChildMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    /// Child is a leaf
    Leaf(&'a mut C::Leaf),
    /// Child is an annotated node
    Node(&'a mut Annotated<C, S>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// A type that can recursively contain itself and leaves.
pub trait Compound<S>
where
    Self: Canon<S>,
    S: Store,
{
    /// The leaf type of the Compound collection
    type Leaf;

    /// The annotation type of the compound.
    type Annotation: Canon<S>;

    /// Returns a reference to a possible child at specified offset
    fn child(&self, ofs: usize) -> Child<Self, S>
    where
        Self: Compound<S>;

    /// Returns a mutable reference to a possible child at specified offset
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, S>
    where
        Self: Compound<S>;
}

/// Find the nth element of any collection satisfying the given annotation
/// constraints
pub trait Nth<'a, S>
where
    Self: Compound<S>,
    Self::Annotation: Annotation<Self, Self::Leaf>,
    S: Store,
{
    /// Construct a `Branch` pointing to the `nth` element, if any
    fn nth(&'a self, n: u64) -> Result<Option<Branch<'a, Self, S>>, S::Error>;

    /// Construct a `BranchMut` pointing to the `nth` element, if any
    fn nth_mut(
        &'a mut self,
        n: u64,
    ) -> Result<Option<BranchMut<'a, Self, S>>, S::Error>;
}

impl<'a, C, S> Nth<'a, S> for C
where
    C: Compound<S>,
    C::Annotation: Annotation<C, C::Leaf> + Borrow<Cardinality>,
    S: Store,
{
    fn nth(
        &'a self,
        mut index: u64,
    ) -> Result<Option<Branch<'a, Self, S>>, S::Error> {
        Branch::walk(self, |f| match f {
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
    ) -> Result<Option<BranchMut<'a, Self, S>>, S::Error> {
        BranchMut::walk(self, |f| match f {
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
