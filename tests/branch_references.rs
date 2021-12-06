// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod sorted_tree;
use sorted_tree::NaiveMap;

use microkelvin::{BranchRef, HostStore, MaxKey};
use rkyv::rend::LittleEndian;
use rkyv::Archive;

#[test]
fn branch_ref() {
    let mut map = NaiveMap::<_, _, MaxKey<_>, HostStore>::new();

    let n = 64;

    for i in 0..n {
        let key: LittleEndian<u64> = i.into();
        map.insert(key, i + 1);
    }

    for i in 0..n {
        let key: LittleEndian<u64> = i.into();
        let branch = map.get(&key).unwrap();
        assert_eq!(branch.leaf(), i + 1);
    }
}
