// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::boxed::Box;

use ranno::Annotated;

/// The response of the [`child`] method on a [`Compound`] node.
///
/// [`child`]: Compound::child
#[derive(Debug)]
pub enum Child<'a, C, A>
where
    C: Compound<A>,
{
    /// Child is a leaf
    Leaf(&'a C::Leaf),
    /// Child is an annotated subtree node
    Node(&'a Annotated<Box<C>, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// The response of the [`child_mut`] method on a [`Compound`] node.
///
/// [`child_mut`]: Compound::child_mut
#[derive(Debug)]
pub enum ChildMut<'a, C, A>
where
    C: Compound<A>,
{
    /// Child is a leaf
    Leaf(&'a mut C::Leaf),
    /// Child is an annotated node
    Node(&'a mut Annotated<Box<C>, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// A type that can recursively contain itself and leaves.
pub trait Compound<A>: Sized {
    /// The leaf type of the compound collection
    type Leaf;

    /// Returns a reference to a possible child at specified index
    fn child(&self, index: usize) -> Child<Self, A>;

    /// Returns a mutable reference to a possible child at specified index
    fn child_mut(&mut self, index: usize) -> ChildMut<Self, A>;
}

/// Marker trait to signal that a data structure can allow mutable access to
/// its leaves.
///
/// For example, a `Vec`-like structure can allow editing of its leaves without
/// issue, whereas editing the (Key, Value) pair of a map could make the map
/// logically invalid.
///
/// Note that this is still safe to implement, since it can only cause logical
/// errors, not undefined behaviour,
pub trait MutableLeaves {}
