// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;
use rkyv::{Archive, Deserialize, Serialize};
use std::cmp::Ordering;

use microkelvin::{
    Annotation, ArchivedChild, ArchivedCompound, Child, ChildMut, Compound,
    Link, MaxKey, Store,
};

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive(bound(serialize = "
  A: Archive + Clone + Annotation<T>,
  T: Clone,
  S: Store<Storage = __S>,"))]
#[archive(bound(deserialize = "
  T: Archive + Clone,
  T::Archived: Deserialize<T, S>,
  A: Clone + Annotation<T>,
  for<'a> &'a mut __D: Borrow<S>,
  __D: Store"))]
enum NaiveTree<T, A, S>
where
    S: Store,
{
    Empty,
    Single(T),
    Double(T, T),
    Middle(
        #[omit_bounds] Link<NaiveTree<T, A, S>, A, S>,
        T,
        #[omit_bounds] Link<NaiveTree<T, A, S>, A, S>,
    ),
}

impl<T, A, S> Default for NaiveTree<T, A, S>
where
    S: Store,
{
    fn default() -> Self {
        NaiveTree::Empty
    }
}

impl<T, A, S> Compound<A, S> for NaiveTree<T, A, S>
where
    S: Store,
    T: Archive,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A, S> {
        match (ofs, self) {
            (0, NaiveTree::Single(a)) => Child::Leaf(a),

            (0, NaiveTree::Double(a, _)) => Child::Leaf(a),
            (1, NaiveTree::Double(_, b)) => Child::Leaf(b),

            (0, NaiveTree::Middle(a, _, _)) => Child::Link(a),
            (1, NaiveTree::Middle(_, b, _)) => Child::Leaf(b),
            (2, NaiveTree::Middle(_, _, c)) => Child::Link(c),

            (_, NaiveTree::Empty) | (_, _) => Child::End,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A, S> {
        match (ofs, self) {
            (0, NaiveTree::Single(a)) => ChildMut::Leaf(a),

            (0, NaiveTree::Double(a, _)) => ChildMut::Leaf(a),
            (1, NaiveTree::Double(_, b)) => ChildMut::Leaf(b),

            (0, NaiveTree::Middle(a, _, _)) => ChildMut::Link(a),
            (1, NaiveTree::Middle(_, b, _)) => ChildMut::Leaf(b),
            (2, NaiveTree::Middle(_, _, c)) => ChildMut::Link(c),

            (_, NaiveTree::Empty) | (_, _) => ChildMut::End,
        }
    }
}

impl<T, A, S> ArchivedCompound<NaiveTree<T, A, S>, A, S>
    for ArchivedNaiveTree<T, A, S>
where
    S: Store,
    T: Archive,
{
    fn child(&self, ofs: usize) -> ArchivedChild<NaiveTree<T, A, S>, A, S> {
        match (ofs, self) {
            (0, ArchivedNaiveTree::Single(t)) => ArchivedChild::Leaf(t),

            (0, ArchivedNaiveTree::Double(t, _)) => ArchivedChild::Leaf(t),
            (1, ArchivedNaiveTree::Double(_, t)) => ArchivedChild::Leaf(t),

            (0, ArchivedNaiveTree::Middle(a, _, _)) => ArchivedChild::Link(a),
            (1, ArchivedNaiveTree::Middle(_, b, _)) => ArchivedChild::Leaf(b),
            (2, ArchivedNaiveTree::Middle(_, _, c)) => ArchivedChild::Link(c),

            (_, ArchivedNaiveTree::Empty) | (_, _) => ArchivedChild::End,
        }
    }
}

impl<T, A, S> NaiveTree<T, A, S>
where
    S: Store,
    T: Archive + Ord + Clone,
    T::Archived: Deserialize<T, S>,
    A: Annotation<T> + Clone,
    A::Archived: Deserialize<A, S>,
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
}

#[cfg(test)]
mod test {
    use super::*;

    use std::io;

    use microkelvin::{HostStore, Keyed, Member};
    use rand::prelude::SliceRandom;
    use rend::LittleEndian;

    #[derive(
        Ord, PartialOrd, PartialEq, Eq, Archive, Clone, Deserialize, Serialize,
    )]
    struct TestLeaf {
        key: LittleEndian<u16>,
    }

    impl Keyed<LittleEndian<u16>> for TestLeaf {
        fn key(&self) -> &LittleEndian<u16> {
            &self.key
        }
    }

    impl Keyed<LittleEndian<u16>> for ArchivedTestLeaf {
        fn key(&self) -> &LittleEndian<u16> {
            &self.key
        }
    }

    impl TestLeaf {
        fn new(key: u16) -> Self {
            TestLeaf { key: key.into() }
        }
    }

    #[test]
    fn many_many_many() -> Result<(), io::Error> {
        let store = HostStore::new();

        const N: u16 = 1024;

        let mut rng = rand::thread_rng();
        let mut numbers = vec![];

        for i in 0..N {
            numbers.push(i);
        }

        let ordered = numbers.clone();
        numbers.shuffle(&mut rng);

        let mut tree = NaiveTree::<_, MaxKey<LittleEndian<u16>>, _>::new();

        for n in &numbers {
            let leaf = TestLeaf::new(*n);
            tree.insert(leaf);
        }

        for n in &numbers {
            let n: LittleEndian<_> = n.into();
            assert!(tree.walk(Member(&n)).is_some());
        }

        let stored = store.put(&tree);

        for n in ordered {
            let n: LittleEndian<_> = n.into();
            assert!(stored.walk(Member(&n)).is_some());
        }

        Ok(())
    }
}
