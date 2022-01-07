// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;

use bytecheck::CheckBytes;
use rkyv::{
    validation::validators::DefaultValidator, Archive, Deserialize, Serialize,
};

use microkelvin::{
    Annotation, ArchivedChild, ArchivedCompound, BranchRef, BranchRefMut,
    Child, ChildMut, Compound, Discriminant, Keyed, Link, MaxKey,
    MaybeArchived, Step, StoreProvider, StoreRef, StoreSerializer, Walkable,
    Walker,
};

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive(bound(serialize = "
  T: Clone + Serialize<StoreSerializer<I>>, 
  A: Clone + Annotation<T>,
  I: Clone + Default,
  __S: Sized + BorrowMut<StoreSerializer<I>>"))]
#[archive(bound(deserialize = "
  T: Archive + Clone,
  T::Archived: Deserialize<T, StoreRef<I>>,
  A: Clone + Annotation<T>,
  I: Clone, 
  __D: StoreProvider<I>"))]
pub enum NaiveTree<T, A, I> {
    Empty,
    Single(T),
    Double(T, T),
    Middle(
        #[omit_bounds] Link<NaiveTree<T, A, I>, A, I>,
        T,
        #[omit_bounds] Link<NaiveTree<T, A, I>, A, I>,
    ),
}

impl<T, A, I> Default for NaiveTree<T, A, I> {
    fn default() -> Self {
        NaiveTree::Empty
    }
}

impl<T, A, I> Compound<A, I> for NaiveTree<T, A, I>
where
    T: Archive,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A, I> {
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

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A, I> {
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

impl<T, A, I> ArchivedCompound<NaiveTree<T, A, I>, A, I>
    for ArchivedNaiveTree<T, A, I>
where
    T: Archive,
{
    fn child(&self, ofs: usize) -> ArchivedChild<NaiveTree<T, A, I>, A, I> {
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

impl<T, A, I> NaiveTree<T, A, I>
where
    T: Archive + Ord + Clone,
    T::Archived: Deserialize<T, StoreRef<I>>
        + for<'any> CheckBytes<DefaultValidator<'any>>,
    A: Annotation<T> + Clone,
    A::Archived: Deserialize<A, StoreRef<I>>
        + for<'any> CheckBytes<DefaultValidator<'any>>,
    I: Clone + for<'any> CheckBytes<DefaultValidator<'any>>,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, t: T) -> Option<T> {
        match std::mem::take(self) {
            NaiveTree::Empty => {
                *self = NaiveTree::Single(t);
                None
            }

            NaiveTree::Single(a) => match t.cmp(&a) {
                Ordering::Less => {
                    *self = NaiveTree::Double(t, a);
                    None
                }
                Ordering::Equal => {
                    *self = NaiveTree::Single(t);
                    Some(a)
                }
                Ordering::Greater => {
                    *self = NaiveTree::Double(a, t);
                    None
                }
            },
            NaiveTree::Double(a, b) => match (t.cmp(&a), t.cmp(&b)) {
                (Ordering::Equal, _) => {
                    *self = NaiveTree::Double(t, b);
                    Some(a)
                }
                (_, Ordering::Equal) => {
                    *self = NaiveTree::Double(a, t);
                    Some(b)
                }
                (Ordering::Greater, Ordering::Greater) => {
                    *self = NaiveTree::Middle(
                        Link::new(NaiveTree::Single(a)),
                        b,
                        Link::new(NaiveTree::Single(t)),
                    );
                    None
                }
                (Ordering::Less, Ordering::Less) => {
                    *self = NaiveTree::Middle(
                        Link::new(NaiveTree::Single(t)),
                        a,
                        Link::new(NaiveTree::Single(b)),
                    );
                    None
                }
                (Ordering::Greater, Ordering::Less) => {
                    *self = NaiveTree::Middle(
                        Link::new(NaiveTree::Single(a)),
                        t,
                        Link::new(NaiveTree::Single(b)),
                    );
                    None
                }
                _ => unreachable!(),
            },
            NaiveTree::Middle(mut left, mid, mut right) => match t.cmp(&mid) {
                Ordering::Less => {
                    let res = left.inner_mut().insert(t);
                    *self = NaiveTree::Middle(left, mid, right);
                    res
                }
                Ordering::Equal => {
                    *self = NaiveTree::Middle(left, t, right);
                    Some(mid)
                }
                Ordering::Greater => {
                    let res = right.inner_mut().insert(t);
                    *self = NaiveTree::Middle(left, mid, right);
                    res
                }
            },
        }
    }
}

#[derive(Clone, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct KvPair<K, V>(K, V);

impl<K, V> PartialEq for KvPair<K, V>
where
    K: PartialEq,
{
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<K, V> PartialOrd for KvPair<K, V>
where
    K: PartialOrd,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.0.partial_cmp(&other.0)
    }
}

impl<K, V> Ord for KvPair<K, V>
where
    K: Ord,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl<K, V> Eq for KvPair<K, V> where K: Eq {}

impl<K, V> Keyed<K> for KvPair<K, V> {
    fn key(&self) -> &K {
        &self.0
    }
}

impl<K, V> Keyed<K> for ArchivedKvPair<K, V>
where
    K: Archive<Archived = K>,
    V: Archive,
{
    fn key(&self) -> &K {
        &self.0
    }
}

impl<K, V> KvPair<K, V> {
    fn into_val(self) -> V {
        self.1
    }

    fn value(&self) -> &V {
        &self.1
    }

    fn value_mut(&mut self) -> &mut V {
        &mut self.1
    }
}

pub struct NaiveMap<K, V, A, I>(NaiveTree<KvPair<K, V>, A, I>);

struct Lookup<'k, K>(&'k K);

impl<'k, C, A, I, K> Walker<C, A, I> for Lookup<'k, K>
where
    C: Compound<A, I>,
    C::Leaf: Keyed<K>,
    <C::Leaf as Archive>::Archived: Keyed<K>,
    K: Ord,
    A: Borrow<MaxKey<K>>,
{
    fn walk(&mut self, walk: impl Walkable<C, A, I>) -> Step {
        for i in 0.. {
            match walk.probe(i) {
                Discriminant::Leaf(kv) => match self.0.cmp(kv.key()) {
                    Ordering::Less => return Step::Abort,
                    Ordering::Equal => return Step::Found(i),
                    Ordering::Greater => (),
                },
                Discriminant::Annotation(anno) => match (*anno).borrow() {
                    MaxKey::NegativeInfinity => unreachable!(),
                    MaxKey::Maximum(key) => match self.0.cmp(key) {
                        Ordering::Greater => (),
                        Ordering::Equal | Ordering::Less => {
                            return Step::Found(i)
                        }
                    },
                },
                Discriminant::Empty => unreachable!(),
                Discriminant::End => return Step::Abort,
            };
        }
        unreachable!()
    }
}

impl<K, V, A, I> NaiveMap<K, V, A, I>
where
    K: Archive<Archived = K>
        + Ord
        + Clone
        + Deserialize<K, StoreRef<I>>
        + for<'a> CheckBytes<DefaultValidator<'a>>,
    V: Archive + Clone,
    V::Archived:
        Deserialize<V, StoreRef<I>> + for<'a> CheckBytes<DefaultValidator<'a>>,
    A: Annotation<KvPair<K, V>>
        + Deserialize<A, StoreRef<I>>
        + Borrow<MaxKey<K>>
        + for<'a> CheckBytes<DefaultValidator<'a>>,
    KvPair<K, V>: Keyed<K>,
    I: Clone + for<'a> CheckBytes<DefaultValidator<'a>>,
{
    pub fn new() -> Self {
        NaiveMap(NaiveTree::new())
    }

    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.0.insert(KvPair(k, v)).map(KvPair::into_val)
    }

    pub fn get(&self, k: &K) -> Option<impl BranchRef<V>> {
        if let Some(branch) = self.0.walk(Lookup(k)) {
            let mapped =
                branch.map_leaf(|maybe_archived_kv| match maybe_archived_kv {
                    MaybeArchived::Memory(kv) => {
                        MaybeArchived::Memory(kv.value())
                    }
                    _ => todo!(),
                });
            Some(mapped)
        } else {
            None
        }
    }

    pub fn get_mut(&mut self, k: &K) -> Option<impl BranchRefMut<V>> {
        self.0
            .walk_mut(Lookup(k))
            .map(|branch| branch.map_leaf(|kv| kv.value_mut()))
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
    #[archive_attr(derive(CheckBytes))]
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
        let store = StoreRef::new(HostStore::new());

        const N: u16 = 2;

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

        let stored = store.store(&tree);

        for n in ordered {
            let n: LittleEndian<_> = n.into();
            assert!(stored.walk(Member(&n)).is_some());
        }

        Ok(())
    }
}
