// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use microkelvin::{Child, ChildMut, Compound, First, MutableLeaves};
use ranno::{Annotated, Annotation};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct Cardinality(usize);

impl From<usize> for Cardinality {
    fn from(c: usize) -> Self {
        Self(c)
    }
}

impl From<Cardinality> for usize {
    fn from(c: Cardinality) -> Self {
        c.0
    }
}

impl<T> Annotation<LinkedList<T, Cardinality>> for Cardinality {
    fn from_child(t: &LinkedList<T, Cardinality>) -> Self {
        match t {
            LinkedList::Empty => 0.into(),
            LinkedList::Node { next, .. } => Cardinality(next.anno().0 + 1),
        }
    }
}

impl<T> Annotation<LinkedList<T, ()>> for () {
    fn from_child(_: &LinkedList<T, ()>) -> Self {}
}

#[derive(Clone, Debug)]
pub enum LinkedList<T, A>
where
    A: Annotation<Self>,
{
    Empty,
    Node {
        val: T,
        next: Annotated<Box<Self>, A>,
    },
}

impl<T, A> Default for LinkedList<T, A>
where
    A: Annotation<Self>,
{
    fn default() -> Self {
        LinkedList::Empty
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    A: Annotation<Self>,
{
    type Leaf = T;

    fn child(&self, index: usize) -> Child<Self, A> {
        match (self, index) {
            (LinkedList::Node { val, .. }, 0) => Child::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => Child::Node(next),
            (LinkedList::Node { .. }, _) => Child::EndOfNode,
            (LinkedList::Empty, _) => Child::EndOfNode,
        }
    }

    fn child_mut(&mut self, index: usize) -> ChildMut<Self, A> {
        match (self, index) {
            (LinkedList::Node { val, .. }, 0) => ChildMut::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => ChildMut::Node(next),
            (LinkedList::Node { .. }, _) => ChildMut::EndOfNode,
            (LinkedList::Empty, _) => ChildMut::EndOfNode,
        }
    }
}

impl<T, A> MutableLeaves for LinkedList<T, A> where A: Annotation<Self> {}

impl<T, A> LinkedList<T, A>
where
    A: Annotation<Self>,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, t: T) {
        match core::mem::take(self) {
            LinkedList::Empty => {
                *self = LinkedList::Node {
                    val: t,
                    next: Annotated::new(Box::new(LinkedList::Empty)),
                }
            }
            old @ LinkedList::Node { .. } => {
                *self = LinkedList::Node {
                    val: t,
                    next: Annotated::new(Box::new(old)),
                };
            }
        }
    }

    pub fn pop(&mut self) -> Option<T> {
        match core::mem::take(self) {
            LinkedList::Empty => None,
            LinkedList::Node { val: t, next } => {
                let (mut next, _) = next.split();
                core::mem::swap(self, &mut next);
                Some(t)
            }
        }
    }
}

#[test]
fn push() {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }
}

#[test]
fn push_cardinality() {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.push(i)
    }
}

#[test]
fn iterate_map() {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first().expect("Some(brach_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    let mut count = n - 1;

    for leaf in mapped {
        assert_eq!(*leaf, count);

        count = count.saturating_sub(1);
    }
}

#[test]
fn iterate_map_mutable() {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut().expect("Some(branch_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    let mut count = n - 1;

    for leaf in mapped {
        assert_eq!(*leaf, count);

        count = count.saturating_sub(1);
    }
}

#[test]
fn deref_mapped_mutable_branch() {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut().expect("Some(brach_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    assert_eq!(*mapped, 31);
}
