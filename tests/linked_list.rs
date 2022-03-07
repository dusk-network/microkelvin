// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::BorrowMut;

use bytecheck::CheckBytes;
use microkelvin::{
    All, Annotation, ArchivedChild, ArchivedCompound, Cardinality, Child,
    ChildMut, Compound, Fundamental, HostStore, Link, MutableLeaves, Nth,
    StoreProvider, StoreRef, StoreSerializer, WellArchived, WellFormed,
};
use rend::LittleEndian;
use rkyv::{Archive, Deserialize, Serialize};

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive(bound(serialize = "
  T: WellFormed, 
  T::Archived: WellArchived<T>,
  A: Fundamental + Annotation<T>,
  __S: Sized + BorrowMut<StoreSerializer>"))]
#[archive(bound(deserialize = "
  A: Fundamental,
  __D: StoreProvider,"))]
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
{
    fn child(&self, ofs: usize) -> ArchivedChild<LinkedList<T, A>, A> {
        match (ofs, self) {
            (0, ArchivedLinkedList::Node { val, .. }) => {
                ArchivedChild::Leaf(val)
            }
            (1, ArchivedLinkedList::Node { next, .. }) => {
                ArchivedChild::Link(next)
            }
            (
                _,
                ArchivedLinkedList::Node { .. } | ArchivedLinkedList::Empty,
            ) => ArchivedChild::End,
        }
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    T: Archive,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A> {
        match (ofs, self) {
            (0, LinkedList::Node { val, .. }) => Child::Leaf(val),
            (1, LinkedList::Node { next, .. }) => Child::Link(next),
            (_, LinkedList::Node { .. }) => Child::End,
            (_, LinkedList::Empty) => Child::End,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A> {
        match (ofs, self) {
            (0, LinkedList::Node { val, .. }) => ChildMut::Leaf(val),
            (1, LinkedList::Node { next, .. }) => ChildMut::Link(next),
            (_, LinkedList::Node { .. }) => ChildMut::End,
            (_, LinkedList::Empty) => ChildMut::End,
        }
    }
}

impl<T, A> MutableLeaves for LinkedList<T, A> {}

impl<T, A> LinkedList<T, A>
where
    T: WellFormed,
    T::Archived: WellArchived<T>,
    A: Fundamental,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn push(&mut self, t: T) {
        match core::mem::take(self) {
            LinkedList::Empty => {
                *self = LinkedList::Node {
                    val: t,
                    next: Link::new(LinkedList::Empty),
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
        T: Archive + Clone,
        T::Archived: Deserialize<T, StoreRef>,
        A: Archive + Clone + Annotation<T>,
        A::Archived: Deserialize<A, StoreRef>,
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

mod test {
    use super::*;

    #[test]
    fn push() {
        let n: u64 = 1024;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            list.push(i);
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
            let nth = *list.walk(Nth(i.into())).expect("Some(Branch)").leaf();
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
        let n: u64 = 64;

        let mut list = LinkedList::<_, Cardinality>::new();

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            list.push(i)
        }

        for i in 0..n {
            let mut branch = list.walk_mut(Nth(i)).expect("Some(Branch)");
            *branch.leaf_mut() += 1;
        }

        for i in 0..n {
            let nth = *list.walk(Nth(i.into())).expect("Some(Branch)").leaf();
            assert_eq!(nth, n - i)
        }
    }

    #[test]
    fn iterate_immutable() {
        let n: u64 = 1024;

        let mut list = LinkedList::<_, Cardinality>::new();

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            list.push(i)
        }

        // branch from first element

        let branch = list.walk(All).expect("Some(Branch)");

        let mut count = n;

        for leaf in branch {
            count -= 1;
            assert_eq!(*leaf, count);
        }

        // branch from 7th element
        let branch = list.walk_mut(Nth(6)).expect("Some(Branch)");

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
        let branch_mut = list.walk_mut(All).expect("Some(Branch)");

        let mut count = n;

        for leaf in branch_mut {
            *leaf += 1;
        }

        // branch from first element
        let branch = list.walk(All).expect("Some(Branch)");

        for leaf in branch {
            assert_eq!(*leaf, count);

            count -= 1;
        }

        let branch = list.walk(Nth(7)).expect("Some(Branch)");

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
        let branch_mut = list.walk_mut(All).expect("Some(Branch)");

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
        let branch_mut = list.walk_mut(All).expect("Some(Branch)");

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
        let branch_mut = list.walk_mut(All).expect("Some(Branch)");

        let mut mapped = branch_mut.map_leaf(|x| x);

        assert_eq!(mapped.leaf_mut(), &31);
    }

    #[test]
    fn push_nth_persist() {
        let store = StoreRef::new(HostStore::new());

        let n = 16;

        let mut list = LinkedList::<_, Cardinality>::new();

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            list.push(i)
        }

        for i in 0..n {
            let nth = *list.walk(Nth(i.into())).expect("Some(Branch)").leaf();
            assert_eq!(nth, n - i - 1)
        }

        let stored = store.store(&list);

        for i in 0..n {
            let i = LittleEndian::from(i);
            let nth = stored.walk(Nth(i.into())).expect("Some(Branch)");

            assert_eq!(nth.leaf(), (n - i - 1).into())
        }
    }
}
