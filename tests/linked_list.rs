// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use canonical::Canon;
use canonical_derive::Canon;
use microkelvin::{
    Annotated, Annotation, Child, ChildMut, Compound, MutableLeaves,
};

#[derive(Clone, Canon, Debug)]
pub enum LinkedList<T, A> {
    Empty,
    Node { val: T, next: Annotated<Self, A> },
}

impl<T, A> Default for LinkedList<T, A> {
    fn default() -> Self {
        LinkedList::Empty
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    T: Canon,
    A: Canon,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => Child::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => Child::Node(next),
            (LinkedList::Node { .. }, _) => Child::EndOfNode,
            (LinkedList::Empty, _) => Child::EndOfNode,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => ChildMut::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => ChildMut::Node(next),
            (LinkedList::Node { .. }, _) => ChildMut::EndOfNode,
            (LinkedList::Empty, _) => ChildMut::EndOfNode,
        }
    }
}

impl<T, A> MutableLeaves for LinkedList<T, A> {}

impl<T, A> LinkedList<T, A>
where
    Self: Compound<A>,
    A: Annotation<<Self as Compound<A>>::Leaf>,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, t: T) {
        match core::mem::take(self) {
            LinkedList::Empty => {
                *self = LinkedList::Node {
                    val: t,
                    next: Annotated::new(LinkedList::Empty),
                }
            }
            old @ LinkedList::Node { .. } => {
                *self = LinkedList::Node {
                    val: t,
                    next: Annotated::new(old),
                };
            }
        }
    }
}

#[test]
fn insert_nth() {
    let n: u64 = 1024;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.insert(i)
    }

    for i in 0..n {
        assert_eq!(*list.nth(i).unwrap().unwrap(), n - i - 1)
    }
}

#[test]
fn insert_mut() {
    let n: u64 = 1024;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.insert(i)
    }

    for i in 0..n {
        *list.nth_mut(i).unwrap().unwrap() += 1
    }

    for i in 0..n {
        assert_eq!(*list.nth(i).unwrap().unwrap(), n - i)
    }
}

#[test]
fn iterate_immutable() {
    let n: u64 = 4;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.insert(i)
    }

    // branch from first element
    let branch = list.nth(0).unwrap().unwrap();

    let mut count = n;

    for res_leaf in branch {
        let leaf = res_leaf.unwrap();

        println!("found leaf {:?}", leaf);

        count -= 1;

        assert_eq!(*leaf, count);
    }

    // branch from 8th element
    let branch = list.nth(2).unwrap().unwrap();

    println!("branh {:?}", branch);

    let mut count = n - 2;

    for res_leaf in branch {
        let leaf = res_leaf.unwrap();

        println!("2nd found leaf {:?}", leaf);

        count -= 1;

        assert_eq!(*leaf, count);
    }
}

#[test]
fn iterate_mutable() {
    let n: u64 = 32;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.insert(i)
    }

    // branch from first element
    let branch_mut = list.nth_mut(0).unwrap().unwrap();

    let mut count = n;

    for res_leaf in branch_mut {
        *res_leaf.unwrap() += 1;
    }

    // branch from first element
    let branch = list.nth(0).unwrap().unwrap();

    for res_leaf in branch {
        let leaf = res_leaf.unwrap();

        assert_eq!(*leaf, count);

        count -= 1;
    }

    // branch from 8th element
    let branch = list.nth(7).unwrap().unwrap();

    let mut count = n - 7;

    for res_leaf in branch {
        let leaf = res_leaf.unwrap();

        assert_eq!(*leaf, count);

        count -= 1;
    }
}
