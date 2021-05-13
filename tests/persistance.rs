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
    use microkelvin::{BackendCtor, Compound, DiskBackend, Keyed, Persistance};

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

    fn testbackend() -> BackendCtor<DiskBackend> {
        BackendCtor::new(|| {
            let dir = tempfile::tempdir().unwrap();
            DiskBackend::new(dir.path()).unwrap()
        })
    }

    #[test]
    fn persist() {
        let n: u64 = 16;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.push(i);
        }

        println!("list post push {:?}", list);

        let persisted = Persistance::persist(&testbackend(), &list).unwrap();

        println!("list post persisted {:?}", list);

        let restored_generic = persisted.reify().unwrap();

        println!("list post restored generic {:?}", restored_generic);

        let mut restored: LinkedList<u64, ()> =
            LinkedList::from_generic(&restored_generic).unwrap();

        println!("list post restored cast {:?}", restored);

        // first empty the original

        for i in 0..n {
            assert_eq!(list.pop().unwrap(), Some(n - i - 1));
            println!("list A: {:?}", list);
        }

        // then the restored copy

        for i in 0..n {
            assert_eq!(restored.pop().unwrap(), Some(n - i - 1));
            println!("list B: {:?}", restored);
        }
    }
}
