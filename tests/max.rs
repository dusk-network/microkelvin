// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use rand::{prelude::SliceRandom, thread_rng};
use rend::LittleEndian;
use rkyv::{Archive, Deserialize, Serialize};

mod linked_list;
use linked_list::LinkedList;
use microkelvin::{GetMaxKey, Keyed, MaxKey};

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
fn maximum() {
    let n: u64 = 1024;

    let mut keys = vec![];

    for i in 0..n {
        let i: LittleEndian<u64> = i.into();
        keys.push(i)
    }

    keys.shuffle(&mut thread_rng());

    let mut list = LinkedList::<_, MaxKey<LittleEndian<u64>>>::new();

    for key in keys {
        list.push(TestLeaf { key, other: () });
    }

    let max = list.max_key().expect("Some(branch)");

    assert_eq!(
        *max.leaf(),
        TestLeaf {
            key: 1023.into(),
            other: ()
        }
    );
}
