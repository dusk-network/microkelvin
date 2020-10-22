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

pub enum Child<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Leaf(&'a C::Leaf),
    Node(&'a Annotated<C, S>),
    EndOfNode,
}

pub enum ChildMut<'a, C, S>
where
    C: Compound<S>,
    S: Store,
{
    Leaf(&'a mut C::Leaf),
    Node(&'a mut Annotated<C, S>),
    EndOfNode,
}

/// Trait for compound datastructures
pub trait Compound<S>
where
    Self: Canon<S>,
    S: Store,
{
    type Leaf;
    type Annotation: Canon<S> + Clone + Sized;

    fn child(&self, ofs: usize) -> Child<Self, S>;
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, S>;
}

pub trait Nth<'a, S>
where
    Self: Compound<S> + Sized,
    Self::Annotation: Annotation<Self, S>,
    S: Store,
{
    fn nth<const N: usize>(
        &'a self,
        n: u64,
    ) -> Result<Option<Branch<'a, Self, S, N>>, S::Error>;

    fn nth_mut<const N: usize>(
        &'a mut self,
        n: u64,
    ) -> Result<Option<BranchMut<'a, Self, S, N>>, S::Error>;
}

impl<'a, C, S> Nth<'a, S> for C
where
    C: Compound<S>,
    C::Annotation: Annotation<C, S> + Borrow<Cardinality>,
    S: Store,
{
    fn nth<const N: usize>(
        &'a self,
        mut index: u64,
    ) -> Result<Option<Branch<'a, Self, S, N>>, S::Error> {
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
        })
    }

    fn nth_mut<const N: usize>(
        &'a mut self,
        mut index: u64,
    ) -> Result<Option<BranchMut<'a, Self, S, N>>, S::Error> {
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
