// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;

use rand::{prelude::SliceRandom, thread_rng};

mod linked_list;
use linked_list::LinkedList;

use canonical_derive::Canon;
use microkelvin::{
    Annotation, Cardinality, Combine, Compound, GetMaxKey, Keyed, MaxKey,
};

#[derive(Default, Clone, Canon)]
struct Anno<K> {
    max: MaxKey<K>,
    card: Cardinality,
}

impl<K> Borrow<MaxKey<K>> for Anno<K> {
    fn borrow(&self) -> &MaxKey<K> {
        &self.max
    }
}

impl<K> Borrow<Cardinality> for Anno<K> {
    fn borrow(&self) -> &Cardinality {
        &self.card
    }
}

impl<Leaf, K> Annotation<Leaf> for Anno<K>
where
    Leaf: Keyed<K>,
    K: Clone + Ord + Default,
{
    fn from_leaf(leaf: &Leaf) -> Self {
        Anno {
            max: MaxKey::from_leaf(leaf),
            card: Cardinality::from_leaf(leaf),
        }
    }
}

impl<C, A, K> Combine<C, A> for Anno<K>
where
    C: Compound<A>,
    C::Leaf: Keyed<K>,
    A: Annotation<C::Leaf> + Borrow<MaxKey<K>> + Borrow<Cardinality>,
    K: Clone + Ord + Default,
{
    fn combine(node: &C) -> Self {
        Anno {
            max: MaxKey::combine(node),
            card: Cardinality::combine(node),
        }
    }
}

#[derive(PartialEq, Clone, Canon, Debug)]
struct TestLeaf {
    key: u64,
    other: (),
}

impl Keyed<u64> for TestLeaf {
    fn key(&self) -> &u64 {
        &self.key
    }
}

#[test]
fn maximum_multiple() {
    let n: u64 = 1024;

    let mut keys = vec![];

    for i in 0..n {
        keys.push(i)
    }

    keys.shuffle(&mut thread_rng());

    let mut list = LinkedList::<_, Anno<u64>>::new();

    for key in keys {
        list.insert(TestLeaf { key, other: () });
    }

    let max = list.max_key().unwrap().unwrap();

    assert_eq!(
        *max,
        TestLeaf {
            key: 1023,
            other: ()
        }
    )
}
