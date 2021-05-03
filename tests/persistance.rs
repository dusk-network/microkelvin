// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod linked_list;

#[cfg(feature = "persistance")]
mod persist_tests {
    use super::*;

    use linked_list::LinkedList;

    use canonical_derive::Canon;
    use microkelvin::{Keyed, PStore, Persist};

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
    fn persist() {
        let dir = tempfile::tempdir().unwrap();
        let mut store = PStore::new(dir.path()).unwrap();

        let n: u64 = 4;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.insert(i);
        }

        let id = list.persist(&mut store);

        let restored = LinkedList::restore(id, &store);
    }
}
