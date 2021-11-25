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

pub enum Discriminant<'a, L, A>
where
    L: Archive,
{
    Leaf(MaybeArchived<'a, L>),
    Annotation(ARef<'a, A>),
    Empty,
    End,
}

pub trait Walkable<S, C, A>
where
    C: Compound<S, A>,
{
    fn probe(&self, ofs: usize) -> Discriminant<C::Leaf, A>;
}

/// The trait used to construct a `Branch` or to iterate through a tree.
pub trait Walker<S, C, A>
where
    C: Compound<S, A>,
{
    /// Walk the tree node, returning the appropriate `Step`
    fn walk(&mut self, walk: impl Walkable<S, C, A>) -> Step;
}

/// Walker that visits all leaves
#[derive(Debug)]
pub struct All;

impl<S, C, A> Walker<S, C, A> for All
where
    C: Compound<S, A>,
{
    fn walk(&mut self, walk: impl Walkable<S, C, A>) -> Step {
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
