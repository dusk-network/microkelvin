// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;
use rand::{prelude::SliceRandom, thread_rng};

mod linked_list;
use linked_list::LinkedList;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use microkelvin::{
    AnnoIter, Annotation, Cardinality, Combine, Compound, GetMaxKey, Keyed,
    MaxKey,
};

#[derive(Default, Clone, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
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
    K: Ord + Default + Clone,
{
    fn from_leaf(leaf: &Leaf) -> Self {
        Anno {
            max: MaxKey::from_leaf(leaf),
            card: Cardinality::from_leaf(leaf),
        }
    }
}

impl<K, A> Combine<A> for Anno<K>
where
    K: Clone + Ord + Default,
    A: Borrow<MaxKey<K>> + Borrow<Cardinality>,
{
    fn combine<C>(iter: AnnoIter<C, A>) -> Self
    where
        C: Compound<A>,
        A: Annotation<C::Leaf>,
    {
        Anno {
            max: MaxKey::combine(iter.clone()),
            card: Cardinality::combine(iter.clone()),
        }
    }
}

#[derive(PartialEq, Clone, Debug, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
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
        list.push(TestLeaf { key, other: () });
    }

    let max = list.max_key().expect("Some(branch)");

    assert_eq!(
        *max,
        TestLeaf {
            key: 1023,
            other: ()
        }
    );
}
