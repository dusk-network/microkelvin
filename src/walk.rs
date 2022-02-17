// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::Archive;

use crate::{ARef, Compound, MaybeArchived};

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

/// Information to select paths in searches
pub enum Discriminant<'a, L, A>
where
    L: Archive,
{
    /// Search encountered a leaf
    Leaf(MaybeArchived<'a, L>),
    /// Search encountered an annotated subtree
    Annotation(ARef<'a, A>),
    /// Search encountered an empty slot
    Empty,
    /// Search encountered the end of the node
    End,
}

/// Wrapper trait provided to Walkers
pub trait Walkable<C, A>
where
    C: Compound<A>,
    C::Leaf: Archive,
{
    /// Probe the location of the tree being walked
    fn probe(&self, ofs: usize) -> Discriminant<C::Leaf, A>;
}

/// The trait used to construct a `Branch` or to iterate through a tree.
pub trait Walker<C, A>
where
    C: Compound<A>,
    C::Leaf: Archive,
{
    /// Walk the tree node, returning the appropriate `Step`
    fn walk(&mut self, walk: impl Walkable<C, A>) -> Step;
}

/// Walker that visits all leaves
#[derive(Debug)]
pub struct All;

impl<C, A> Walker<C, A> for All
where
    C: Compound<A>,
    C::Leaf: Archive,
{
    fn walk(&mut self, walk: impl Walkable<C, A>) -> Step {
        for i in 0.. {
            match walk.probe(i) {
                Discriminant::End => return Step::Advance,
                Discriminant::Empty => (),
                _ => return Step::Found(i),
            }
        }
        unreachable!()
    }
}
