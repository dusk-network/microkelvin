// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::{Archive, Deserialize};

use crate::link::{ArchivedLink, Link};
use crate::storage::StoreRef;
use crate::tower::{WellArchived, WellFormed};
use crate::{Annotation, Branch, BranchMut, MaybeArchived, Walker};

/// The response of the `child` method on a `Compound` node.
pub enum Child<'a, C, A>
where
    C: Compound<A>,
{
    /// Child is a leaf
    Leaf(&'a C::Leaf),
    /// Child is an annotated subtree node
    Link(&'a Link<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    End,
}

/// The response of the `child` method on a `Compound` node.
pub enum ArchivedChild<'a, C, A>
where
    C: Compound<A>,
    C::Leaf: Archive,
{
    /// Child is a leaf
    Leaf(&'a <C::Leaf as Archive>::Archived),
    /// Child is an annotated subtree node
    Link(&'a ArchivedLink<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    End,
}

/// The response of the `child_mut` method on a `Compound` node.
pub enum ChildMut<'a, C, A>
where
    C: Compound<A>,
{
    /// Child is a leaf
    Leaf(&'a mut C::Leaf),
    /// Child is an annotated node
    Link(&'a mut Link<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    End,
}

/// Trait to support branch traversal in archived nodes
pub trait ArchivedCompound<C, A>
where
    C: Compound<A>,
    C::Leaf: Archive,
{
    /// Returns an archived child
    fn child(&self, ofs: usize) -> ArchivedChild<C, A>;
}

/// A type that can recursively contain itself and leaves.
pub trait Compound<A>: Sized {
    /// The leaf type of the Compound collection
    type Leaf;

    /// Get a reference to a child    
    fn child(&self, ofs: usize) -> Child<Self, A>;

    /// Get a mutable reference to a child
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>;

    /// Constructs a branch from this root compound
    fn walk<'a, W>(&'a self, walker: W) -> Option<Branch<'a, Self, A>>
    where
        W: Walker<Self, A>,
        Self: WellFormed,
        Self::Archived: ArchivedCompound<Self, A> + WellArchived<Self>,
        Self::Leaf: 'a + WellFormed,
        <Self::Leaf as Archive>::Archived: WellArchived<Self::Leaf>,
        A: Annotation<Self::Leaf>,
    {
        Branch::walk(MaybeArchived::Memory(self), walker)
    }

    /// Constructs a mutable branch from this root compound    
    fn walk_mut<'a, W>(
        &'a mut self,
        walker: W,
    ) -> Option<BranchMut<'a, Self, A>>
    where
        Self: WellFormed,
        Self::Archived: Deserialize<Self, StoreRef> + WellArchived<Self>,
        Self::Leaf: WellFormed,
        <Self::Leaf as Archive>::Archived: WellArchived<Self::Leaf>,
        A: Annotation<Self::Leaf>,
        W: Walker<Self, A>,
    {
        BranchMut::walk(self, walker)
    }
}

/// Marker trait to signal that a datastructre can allow mutable access to its
/// leaves.
///
/// For example, a `Vec`-like structure can allow editing of its leaves without
/// issue, whereas editing the (Key, Value) pair of a map could make the map
/// logically invalid.
///
/// Note that this is still safe to implement, since it can only cause logical
/// errors, not undefined behaviour,
pub trait MutableLeaves {}
