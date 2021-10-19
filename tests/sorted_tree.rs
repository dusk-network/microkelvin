// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rkyv::{Archive, Deserialize, Infallible, Serialize};
use std::cmp::Ordering;

use microkelvin::{
    AWrap, Annotation, ArchivedChild, ArchivedCompound, Child, ChildMut,
    Compound, Link, Portal, Storage, StorageSerializer,
};

impl<T, A> Default for NaiveTree<T, A> {
    fn default() -> Self {
        NaiveTree::Empty
    }
}

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive(bound(serialize = "
  T: Serialize<Storage>,
  A: Annotation<T>,
  __S: StorageSerializer"))]
#[archive(bound(deserialize = "
  A: Archive + Clone,
  T::Archived: Deserialize<T, __D>,
  A::Archived: Deserialize<A, __D>,
  __D: Sized"))]
enum NaiveTree<T, A> {
    Empty,
    Single(T),
    Double(T, T),
    Middle(
        #[omit_bounds] Link<NaiveTree<T, A>, A>,
        T,
        #[omit_bounds] Link<NaiveTree<T, A>, A>,
    ),
}

impl<T, A> Compound<A> for NaiveTree<T, A>
where
    T: Archive,
    A: Annotation<T>,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A> {
        match (ofs, self) {
            (0, NaiveTree::Single(a)) => Child::Leaf(a),
            (0, NaiveTree::Double(a, _)) => Child::Leaf(a),
            (1, NaiveTree::Double(_, b)) => Child::Leaf(b),
            (0, NaiveTree::Middle(a, _, _)) => Child::Node(a),
            (1, NaiveTree::Middle(_, b, _)) => Child::Leaf(b),
            (2, NaiveTree::Middle(_, _, c)) => Child::Node(c),
            (_, NaiveTree::Empty) | (_, _) => Child::EndOfNode,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A> {
        match (ofs, self) {
            (0, NaiveTree::Single(a)) => ChildMut::Leaf(a),
            (0, NaiveTree::Double(a, _)) => ChildMut::Leaf(a),
            (1, NaiveTree::Double(_, b)) => ChildMut::Leaf(b),
            (0, NaiveTree::Middle(a, _, _)) => ChildMut::Node(a),
            (1, NaiveTree::Middle(_, b, _)) => ChildMut::Leaf(b),
            (2, NaiveTree::Middle(_, _, c)) => ChildMut::Node(c),
            (_, NaiveTree::Empty) | (_, _) => ChildMut::EndOfNode,
        }
    }
}

impl<T, A> ArchivedCompound<NaiveTree<T, A>, A> for ArchivedNaiveTree<T, A>
where
    T: Archive,
    A: Annotation<T>,
{
    fn child(&self, ofs: usize) -> ArchivedChild<NaiveTree<T, A>, A> {
        match (ofs, self) {
            (0, ArchivedNaiveTree::Single(t)) => ArchivedChild::Leaf(t),
            (1, ArchivedNaiveTree::Double(_, b)) => ArchivedChild::Leaf(b),
            (0, ArchivedNaiveTree::Middle(a, _, _)) => ArchivedChild::Node(a),
            (1, ArchivedNaiveTree::Middle(_, b, _)) => ArchivedChild::Leaf(b),
            (2, ArchivedNaiveTree::Middle(_, _, c)) => ArchivedChild::Node(c),
            (_, ArchivedNaiveTree::Empty) | (_, _) => ArchivedChild::EndOfNode,
        }
    }
}

impl<T, A> NaiveTree<T, A>
where
    T: Archive + Ord + Clone,
    T::Archived: Deserialize<T, Infallible>,
    A: Annotation<Self> + Clone,
{
    fn new() -> Self {
        Default::default()
    }

    fn insert(&mut self, t: T) {
        match std::mem::take(self) {
            NaiveTree::Empty => *self = NaiveTree::Single(t),

            NaiveTree::Single(a) => {
                *self = match t.cmp(&a) {
                    Ordering::Less => NaiveTree::Double(t, a),
                    Ordering::Equal => NaiveTree::Single(a),
                    Ordering::Greater => NaiveTree::Double(a, t),
                }
            }
            NaiveTree::Double(a, b) => {
                *self = match (t.cmp(&a), t.cmp(&b)) {
                    (Ordering::Equal, _) | (_, Ordering::Equal) => {
                        NaiveTree::Double(a, b)
                    }
                    (Ordering::Greater, Ordering::Greater) => {
                        NaiveTree::Middle(
                            Link::new(NaiveTree::Single(a)),
                            b,
                            Link::new(NaiveTree::Single(t)),
                        )
                    }
                    (Ordering::Less, Ordering::Less) => NaiveTree::Middle(
                        Link::new(NaiveTree::Single(t)),
                        a,
                        Link::new(NaiveTree::Single(b)),
                    ),
                    (Ordering::Greater, Ordering::Less) => NaiveTree::Middle(
                        Link::new(NaiveTree::Single(a)),
                        t,
                        Link::new(NaiveTree::Single(b)),
                    ),
                    _ => unreachable!(),
                }
            }
            NaiveTree::Middle(mut left, mid, mut right) => {
                *self = match t.cmp(&mid) {
                    Ordering::Less => {
                        left.inner_mut().insert(t);
                        NaiveTree::Middle(left, mid, right)
                    }
                    Ordering::Equal => NaiveTree::Middle(left, mid, right),
                    Ordering::Greater => {
                        right.inner_mut().insert(t);
                        NaiveTree::Middle(left, mid, right)
                    }
                }
            }
        }
    }

    fn member(&self, t: &T) -> bool
    where
        T::Archived: PartialEq<T> + PartialOrd<T>,
    {
        match self {
            NaiveTree::Empty => false,
            NaiveTree::Single(a) => a == t,
            NaiveTree::Double(a, b) => a == t || b == t,
            NaiveTree::Middle(left, mid, right) => match t.cmp(&mid) {
                Ordering::Less => match left.inner() {
                    AWrap::Memory(left) => left.member(t),
                    AWrap::Archived(a_left) => a_left.member(t),
                },
                Ordering::Equal => true,
                Ordering::Greater => match right.inner() {
                    AWrap::Memory(right) => right.member(t),
                    AWrap::Archived(a_right) => a_right.member(t),
                },
            },
        }
    }
}

impl<T, A> ArchivedNaiveTree<T, A>
where
    T: Archive + Ord + Clone,
    T::Archived: Deserialize<T, Infallible>,
    A: Annotation<NaiveTree<T, A>> + Clone,
{
    fn member(&self, t: &T) -> bool
    where
        T::Archived: PartialOrd<T>,
    {
        match self {
            ArchivedNaiveTree::Empty => false,
            ArchivedNaiveTree::Single(a) => a == t,
            ArchivedNaiveTree::Double(a, b) => a == t || b == t,
            ArchivedNaiveTree::Middle(left, mid, right) => {
                match mid.partial_cmp(t) {
                    Some(Ordering::Less) => right.inner().member(t),
                    Some(Ordering::Equal) => true,
                    Some(Ordering::Greater) => left.inner().member(t),
                    None => todo!(),
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io;

    use rand::prelude::SliceRandom;
    use rend::LittleEndian;

    #[test]
    fn many_many_many() -> Result<(), io::Error> {
        const N: u16 = 1024;

        let mut rng = rand::thread_rng();
        let mut numbers = vec![];

        for i in 0..N {
            let i: LittleEndian<u16> = i.into();
            numbers.push(i);
        }

        let ordered = numbers.clone();
        numbers.shuffle(&mut rng);

        let mut tree = NaiveTree::<LittleEndian<u16>, ()>::new();

        for n in &numbers {
            let n: LittleEndian<_> = (*n).into();
            tree.insert(n);
        }

        for n in &numbers {
            let n: LittleEndian<_> = (*n).into();
            assert_eq!(tree.member(&n), true)
        }

        let ofs = Portal::put(&tree);

        let archived_tree = Portal::get(ofs);

        for n in &ordered {
            let n: LittleEndian<_> = (*n).into();
            assert_eq!(archived_tree.member(&n), true)
        }

        Ok(())
    }
}
