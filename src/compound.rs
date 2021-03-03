// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::annotations::Annotated;

/// The response of the `child` method on a `Compound` node.
pub enum Child<'a, C, A>
where
    C: Compound,
{
    /// Child is a leaf
    Leaf(&'a C::Leaf),
    /// Child is an annotated subtree node
    Node(&'a Annotated<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// The response of the `child_mut` method on a `Compound` node.
pub enum ChildMut<'a, C, A>
where
    C: Compound,
{
    /// Child is a leaf
    Leaf(&'a mut C::Leaf),
    /// Child is an annotated node
    Node(&'a mut Annotated<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// A type that can recursively contain itself and leaves.
pub trait Compound: Sized {
    /// The leaf type of the Compound collection
    type Leaf;

    /// Returns a reference to a possible child at specified offset
    fn child<A>(&self, ofs: usize) -> Child<Self, A>;

    /// Returns a mutable reference to a possible child at specified offset
    fn child_mut<A>(&mut self, ofs: usize) -> ChildMut<Self, A>;
}
