// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserializeved.

use microkelvin::{
    Annotation, ArchivedChild, ArchivedCompound, Branch, BranchMut,
    Cardinality, Child, ChildMut, Compound, First, Link, MutableLeaves, Nth,
    Portal, PortalDeserializer, Storage, StorageSerializer,
};
use rend::LittleEndian;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive(bound(serialize = "
  T: Serialize<Storage>,
  A: Annotation<T>,
  __S: StorageSerializer"))]
#[archive(bound(deserialize = "
  A: Archive + Clone,
  T::Archived: Deserialize<T, __D>,
  A::Archived: Deserialize<A, __D>,
  __D: PortalDeserializer"))]
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

impl<T, A> ArchivedCompound<LinkedList<T, A>, A> for ArchivedLinkedList<T, A>
where
    T: Archive,
    T::Archived: Deserialize<T, Portal>,
    A::Archived: Deserialize<A, Portal>,
    A: Annotation<T>,
{
    fn child(&self, ofs: usize) -> ArchivedChild<LinkedList<T, A>, A> {
        match (self, ofs) {
            (ArchivedLinkedList::Node { val, .. }, 0) => {
                ArchivedChild::Leaf(val)
            }
            (ArchivedLinkedList::Node { next, .. }, 1) => {
                ArchivedChild::Node(next)
            }
            (ArchivedLinkedList::Node { .. }, _) => ArchivedChild::EndOfNode,
            (ArchivedLinkedList::Empty, _) => ArchivedChild::EndOfNode,
        }
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    T: Archive,
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
    let portal = Portal::new();

    let n = 1024;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    for i in 0..n {
        let nth = *Branch::walk(&list, &portal, Nth::new(i))
            .expect("Some(Branch)")
            .leaf();
        assert_eq!(nth, n - i - 1)
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
    let portal = Portal::new();

    let n: u64 = 64;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        let mut nth =
            BranchMut::walk(&mut list, Nth::new(i)).expect("Some(Branch)");
        *nth += 1;
    }

    for i in 0..n {
        let nth = *Branch::walk(&list, &portal, Nth::new(i))
            .expect("Some(Branch)")
            .leaf();
        assert_eq!(nth, n - i)
    }
}

#[test]
fn iterate_immutable() {
    let portal = Portal::new();

    let n: u64 = 1024;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    // branch from first element

    let branch = Branch::walk(&list, &portal, First).expect("Some(Branch)");

    let mut count = n;

    for leaf in branch {
        count -= 1;
        assert_eq!(*leaf, count);
    }

    // branch from 7th element
    let branch = BranchMut::walk(&mut list, Nth::new(6)).expect("Some(Branch)");

    let mut count = n - 6;

    for leaf in branch {
        count -= 1;
        assert_eq!(*leaf, count);
    }
}

#[test]
fn iterate_mutable() {
    let portal = Portal::new();

    let n: u64 = 32;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    // branch from first element
    let branch_mut = BranchMut::walk(&mut list, First).expect("Some(Branch)");

    let mut count = n;

    for leaf in branch_mut {
        *leaf += 1;
    }

    // branch from first element
    let branch = Branch::walk(&list, &portal, First).expect("Some(Branch)");

    for leaf in branch {
        assert_eq!(*leaf, count);

        count -= 1;
    }

    let branch =
        Branch::walk(&list, &portal, Nth::new(7)).expect("Some(Branch)");

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
    let branch_mut = BranchMut::walk(&mut list, First).expect("Some(Branch)");

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
    let branch_mut = BranchMut::walk(&mut list, First).expect("Some(Branch)");

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
    let branch_mut = BranchMut::walk(&mut list, First).expect("Some(Branch)");

    let mapped = branch_mut.map_leaf(|x| x);

    assert_eq!(core::ops::Deref::deref(&mapped), &31);
}

#[test]
fn push_nth_persist() {
    let portal = Portal::default();

    let n = 10;

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        list.push(i)
    }

    for i in 0..n {
        let nth = *Branch::walk(&list, &portal, Nth::new(i))
            .expect("Some(Branch)")
            .leaf();

        assert_eq!(nth, n - i - 1)
    }

    let stored = portal.put(&list);

    let restored = stored.restore();

    for i in 0..n {
        let nth = *Branch::walk(&restored, &portal, Nth::new(i))
            .expect("Some(Branch)")
            .leaf();

        assert_eq!(nth, n - i - 1)
    }
}
