// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use microkelvin::{
    Annotation, ArchivedChild, ArchivedChildren, Cardinality, Child, ChildMut,
    Compound, First, Link, MutableLeaves, Nth, Portal, PortalProvider,
};
use rend::LittleEndian;
use rkyv::{ser::Serializer, AlignedVec, Archive, Deserialize, Serialize};

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive(bound(archive = "
  A: Archive<Archived = A> + Annotation<T> + Clone,
  T: Archive<Archived = T>"))]
#[archive(bound(serialize = "
  A: Archive<Archived = A> + Serialize<__S>, 
  __S: Serializer + PortalProvider + From<Portal> + Into<AlignedVec>"))]
#[archive(bound(deserialize = "
  A: Archive<Archived = A>,
  A: Deserialize<A, __D>,
  __D: Sized + PortalProvider,
  "))]
pub enum LinkedList<T, A> {
    Empty,
    Node {
        val: T,
        #[omit_bounds]
        next: Link<Self, A>,
    },
}

impl<T, A> Default for LinkedList<T, A> {
    fn default() -> Self {
        LinkedList::Empty
    }
}

impl<T, A> ArchivedChildren<LinkedList<T, A>, A> for ArchivedLinkedList<T, A>
where
    T: Archive<Archived = T>,
    A: Annotation<T>,
{
    fn archived_child(
        &self,
        _ofs: usize,
    ) -> ArchivedChild<LinkedList<T, A>, A> {
        todo!()
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    T: Archive<Archived = T>,
    A: Annotation<T>,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A> {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => Child::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => Child::Node(next),
            (LinkedList::Node { .. }, _) => Child::EndOfNode,
            (LinkedList::Empty, _) => Child::EndOfNode,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A> {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => ChildMut::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => ChildMut::Node(next),
            (LinkedList::Node { .. }, _) => ChildMut::EndOfNode,
            (LinkedList::Empty, _) => ChildMut::EndOfNode,
        }
    }
}

impl<T, A> MutableLeaves for LinkedList<T, A> where A: Archive + Annotation<T> {}

impl<T, A> LinkedList<T, A>
where
    T: Archive<Archived = T>,
    A: Annotation<T>,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, t: T) {
        match core::mem::take(self) {
            LinkedList::Empty => {
                *self = LinkedList::Node {
                    val: t,
                    next: Link::new(LinkedList::<T, A>::Empty),
                }
            }
            old @ LinkedList::Node { .. } => {
                *self = LinkedList::Node {
                    val: t,
                    next: Link::new(old),
                };
            }
        }
    }

    pub fn pop(&mut self) -> Option<T>
    where
        T: Clone,
        A: Clone,
    {
        match core::mem::take(self) {
            LinkedList::Empty => None,
            LinkedList::Node { val: t, next } => {
                *self = next.unlink();
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
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }
}

#[test]
fn push_cardinality() {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }
}

#[test]
fn push_nth() {
    let n = 1024;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    for i in 0..n {
        assert_eq!(*list.nth(i).expect("Some(branch)"), n - i - 1)
    }
}

#[test]
fn push_pop() {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    for i in 0..n {
        assert_eq!(list.pop(), Some((n - i - 1).into()))
    }
}

#[test]
fn push_mut() {
    let n: u64 = 1024;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        *list.nth_mut(i).expect("Some(branch)") += 1
    }

    for i in 0..n {
        assert_eq!(*list.nth(i).expect("Some(branch)"), n - i)
    }
}

#[test]
fn iterate_immutable() {
    // todo - make larger
    let n: u64 = 1;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    println!("joru");

    // branch from first element
    let branch = list.first().expect("Some(branch)");

    println!("skoru");

    let mut count = n;

    for leaf in branch {
        count -= 1;
        assert_eq!(*leaf, count);
    }

    // branch from 7th element
    let branch = list.nth(6).expect("Some(branch)");

    let mut count = n - 6;

    for leaf in branch {
        count -= 1;
        assert_eq!(*leaf, count);
    }
}

#[test]
fn iterate_mutable() {
    let n: u64 = 32;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut().expect("Some(branch_mut)");

    let mut count = n;

    for leaf in branch_mut {
        *leaf += 1;
    }

    // branch from first element
    let branch = list.first().expect("Some(brach)");

    for leaf in branch {
        assert_eq!(*leaf, count);

        count -= 1;
    }

    // branch from 8th element
    let branch = list.nth(7).expect("Some(branch)");

    let mut count = n - 7;

    for leaf in branch {
        assert_eq!(*leaf, count);

        count -= 1;
    }
}

#[test]
fn iterate_map() {
    let n: u64 = 32;

    let mut list = LinkedList::<_, ()>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
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
        let i: LittleEndian<u64> = i.into();
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
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    // branch from first element
    let branch_mut = list.first_mut().expect("Some(brach_mut)");
    let mapped = branch_mut.map_leaf(|x| x);

    assert_eq!(core::ops::Deref::deref(&mapped), &31);
}
