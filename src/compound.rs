// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use crate::annotations::{Ann, Annotated, Annotation};
use alloc::vec::Vec;
use canonical::Canon;

/// The response of the `child` method on a `Compound` node.
pub enum Child<'a, C, A>
where
    C: Compound<A>,
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
    C: Compound<A>,
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
pub trait Compound<A>: Sized + Canon {
    /// The leaf type of the Compound collection
    type Leaf;

    /// Returns a reference to a possible child at specified offset
    fn child(&self, ofs: usize) -> Child<Self, A>
    where
        A: Annotation<Self::Leaf>;

    /// Returns a mutable reference to a possible child at specified offset
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>
    where
        A: Annotation<Self::Leaf>;

    /// Calculate the Compound annotation for a node
    fn annotate_node(&self) -> A
    where
        A: Annotation<Self::Leaf>,
    {
        // default impl allocates, and could be optimized for individual
        // compound types

        let mut children = Vec::new();

        for i in 0.. {
            match self.child(i) {
                Child::Leaf(l) => children.push(Ann::Owned(A::from_leaf(l))),
                Child::Node(n) => children.push(Ann::Borrowed(n.annotation())),
                Child::Empty => (),
                Child::EndOfNode => break,
            }
            let n = 1024;
            debug_assert!(
                i < n,
                "Annotation threshold exceeded after {} iterations.",
                n
            );
        }
        A::combine(&children[..])
    }
}

/// Marker trait to signal that a datastructre can allow mutable access to its
/// leaves.
///
/// For example, a `Vec`-like structure can allow editing of its leaves without
/// issue, whereas editing the (Key, Value) pair of a map could make the map
/// logically invalid.
///
/// Note that this is safe to implement, since it still cannot cause undefined
/// behaviour, only logical errors
pub trait MutableLeaves {}
