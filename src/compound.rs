// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;

use rkyv::{Archive, Deserialize, Infallible};

use crate::annotations::{ARef, Annotation};
use crate::link::{ArchivedLink, Link};
use crate::storage::Stored;

/// The response of the `child` method on a `Compound` node.
pub enum Child<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Child is a leaf
    Leaf(&'a C::Leaf),
    /// Child is an annotated subtree node
    Node(&'a Link<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// The response of the `child` method on a `Compound` node.
pub enum ArchivedChild<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Child is a leaf
    Leaf(&'a <C::Leaf as Archive>::Archived),
    /// Child is an annotated subtree node
    Node(&'a ArchivedLink<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// The response of the `child_mut` method on a `Compound` node.
#[derive(Debug)]
pub enum ChildMut<'a, C, A>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Child is a leaf
    Leaf(&'a mut C::Leaf),
    /// Child is an annotated node
    Node(&'a mut Link<C, A>),
    /// Empty slot
    Empty,
    /// No more children
    EndOfNode,
}

/// Trait to support branch traversal in archived nodes
pub trait ArchivedCompound<C, A>: Deserialize<C, Infallible>
where
    C: Compound<A>,
    A: Annotation<C::Leaf>,
{
    /// Returns an archived child
    fn child(&self, ofs: usize) -> ArchivedChild<C, A>;
}

/// A type that can recursively contain itself and leaves.
pub trait Compound<A>: Sized
where
    A: Annotation<Self::Leaf>,
{
    /// The leaf type of the Compound collection
    type Leaf: Archive;

    /// Get a reference to a child    
    fn child(&self, ofs: usize) -> Child<Self, A>;

    /// Get a mutable reference to a child
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>;

    /// Provides an iterator over all sub-annotations of the compound node
    fn annotations(&self) -> AnnoIter<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        AnnoIter {
            node: self,
            ofs: 0,
            _marker: PhantomData,
        }
    }
}

/// An iterator over the sub-annotations of a Compound collection
#[derive(Debug)]
pub struct AnnoIter<'a, C, A> {
    node: &'a C,
    ofs: usize,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Clone for AnnoIter<'a, C, A> {
    fn clone(&self) -> Self {
        AnnoIter {
            node: self.node,
            ofs: self.ofs,
            _marker: self._marker,
        }
    }
}

impl<'a, C, A> Iterator for AnnoIter<'a, C, A>
where
    A: 'a,
    C: Archive + Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    type Item = ARef<'a, A>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.node.child(self.ofs) {
                Child::Empty => self.ofs += 1,
                Child::EndOfNode => return None,
                Child::Leaf(l) => {
                    self.ofs += 1;
                    return Some(ARef::Owned(A::from_leaf(l)));
                }
                Child::Node(a) => {
                    self.ofs += 1;
                    return Some(a.annotation());
                }
            }
        }
    }
}

impl<C, A> Compound<A> for Stored<C>
where
    C: Compound<A> + Archive,
    C::Archived: ArchivedCompound<C, A>,
    A: Annotation<C::Leaf>,
{
    type Leaf = C::Leaf;

    fn child(&self, _ofs: usize) -> Child<Self, A> {
        todo!()
    }

    fn child_mut(&mut self, _ofs: usize) -> ChildMut<Self, A> {
        todo!()
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
