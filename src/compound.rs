// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::marker::PhantomData;

use crate::annotations::{Annotated, Annotation, WrappedAnnotation};
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

    /// Returns an iterator over the children of the Compound node.
    fn children(&self) -> ChildIterator<Self, A> {
        ChildIterator {
            node: self,
            ofs: 0,
            _marker: PhantomData,
        }
    }
}

/// The kinds of children you can encounter iterating over a Compound
pub enum IterChild<'a, C, A>
where
    C: Compound<A>,
{
    /// Iterator found a leaf
    Leaf(&'a C::Leaf),
    /// Iterator found an annotated node
    Node(&'a Annotated<C, A>),
}

impl<'a, C, A> IterChild<'a, C, A>
where
    A: Annotation<C::Leaf>,
    C: Compound<A>,
{
    /// Returns the annotation of the child
    pub fn annotation(&self) -> WrappedAnnotation<A> {
        match self {
            IterChild::Leaf(l) => WrappedAnnotation::Owned(A::from_leaf(l)),
            IterChild::Node(a) => WrappedAnnotation::Borrowed(a.annotation()),
        }
    }
}

pub struct ChildIterator<'a, C, A> {
    node: &'a C,
    ofs: usize,
    _marker: PhantomData<A>,
}

impl<'a, C, A> Iterator for ChildIterator<'a, C, A>
where
    C: Compound<A>,
    C::Leaf: 'a,
    A: Annotation<C::Leaf> + 'a,
{
    type Item = IterChild<'a, C, A>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.node.child(self.ofs) {
                Child::Empty => self.ofs += 1,
                Child::EndOfNode => return None,
                Child::Leaf(l) => {
                    self.ofs += 1;
                    return Some(IterChild::Leaf(l));
                }
                Child::Node(a) => {
                    self.ofs += 1;
                    return Some(IterChild::Node(a));
                }
            }
        }
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
