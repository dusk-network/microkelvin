// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::Archive;

use crate::annotations::{ARef, Annotation};
use crate::compound::{ArchivedCompound, Compound};

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
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    /// Walk the tree node, returning the appropriate `Step`
    fn walk(&mut self, walk: impl Slots<C, A>) -> Step;
}

pub enum Slot<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    Leaf(&'a C::Leaf),
    ArchivedLeaf(&'a <C::Leaf as Archive>::Archived),
    Annotation(ARef<'a, A>),
    Empty,
    End,
}

pub trait Slots<C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    fn slot(&self, ofs: usize) -> Slot<C, A>;
}

/// Walker that visits all leaves
#[derive(Debug)]
pub struct First;

impl<C, A> Walker<C, A> for First
where
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
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
