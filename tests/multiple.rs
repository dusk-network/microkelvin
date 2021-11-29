// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;
use rand::{prelude::SliceRandom, thread_rng};

mod linked_list;
use linked_list::LinkedList;

use rend::LittleEndian;
use rkyv::{Archive, Deserialize, Serialize};

use microkelvin::{
    Annotation, Cardinality, Combine, Compound, FindMaxKey, HostStore, Keyed,
    MaxKey, Primitive,
};

#[derive(Default, Clone, Archive, Serialize, Debug, Deserialize)]
#[archive(as = "Self")]
#[archive(bound(archive = "
  K: Primitive,
  MaxKey<K>: Primitive,
"))]
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
    K: Primitive + Ord + Default + Clone,
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
    A: Borrow<MaxKey<K>> + Borrow<Cardinality> + Archive<Archived = A>,
{
    fn combine(&mut self, other: &A) {
        self.max.combine(other);
        self.card.combine(other);
    }
}

#[derive(PartialEq, Clone, Debug, Archive, Serialize, Deserialize)]
#[archive(as = "Self")]
struct TestLeaf {
    key: LittleEndian<u64>,
    other: (),
}

impl Keyed<LittleEndian<u64>> for TestLeaf {
    fn key(&self) -> &LittleEndian<u64> {
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

    let mut list = LinkedList::<_, Anno<LittleEndian<u64>>, HostStore>::new();

    for key in keys {
        let key: LittleEndian<u64> = key.into();
        list.push(TestLeaf { key, other: () });
    }

    let max = list.walk(FindMaxKey::default()).expect("Some(Branch)");

    assert_eq!(
        *max.leaf(),
        TestLeaf {
            key: 1023.into(),
            other: ()
        }
    );
}
