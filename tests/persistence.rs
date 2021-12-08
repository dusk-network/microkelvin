// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

mod linked_list;

#[cfg(feature = "host")]
mod persist_tests {
    use super::*;

    use linked_list::LinkedList;

    use std::io;

    use rend::LittleEndian;

    use microkelvin::{Cardinality, HostStore, Keyed, Nth, Store};

    #[derive(PartialEq, Clone, Debug)]
    struct TestLeaf {
        key: u64,
        other: (),
    }

    impl Keyed<u64> for TestLeaf {
        fn key(&self) -> &u64 {
            &self.key
        }
    }

    fn persist() -> Result<(), io::Error> {
        let store = HostStore::new();

        let n: u64 = 16;

        let mut list = LinkedList::<_, Cardinality, _>::new();

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            list.push(i);
        }

        let stored = store.put(&list);

        // first empty the original

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            assert_eq!(list.pop(), Some((n - i - 1).into()));
        }

        // then read from the stored copy

        for i in 0..n {
            let branch = stored.walk(Nth(i)).unwrap();
            assert_eq!(branch.leaf(), (n - i - 1).into());
        }

        Ok(())
    }

    #[test]
    fn persist_a() -> Result<(), io::Error> {
        persist()
    }

    #[test]
    fn persist_b() -> Result<(), io::Error> {
        persist()
    }

    #[test]
    fn persist_c() -> Result<(), io::Error> {
        persist()
    }

    #[test]
    fn persist_d() -> Result<(), io::Error> {
        persist()
    }

    fn persist_across_threads() -> Result<(), io::Error> {
        let store = HostStore::new();

        let n: u64 = 16;

        let mut list = LinkedList::<_, Cardinality, _>::new();

        for i in 0..n {
            let i: LittleEndian<u64> = i.into();
            list.push(i);
        }

        let persisted = store.put(&list);

        // it should now be available from other threads

        std::thread::spawn(move || {
            for i in 0..n {
                let i: LittleEndian<u64> = i.into();
                assert_eq!(
                    *persisted.walk(Nth(i.into())).unwrap().leaf(),
                    n - i - 1
                );
            }
        })
        .join()
        .expect("thread to join cleanly");

        // then empty the original

        for i in 0..n {
            assert_eq!(list.pop(), Some((n - i - 1).into()));
        }

        Ok(())
    }

    #[test]
    fn persist_across_threads_a() -> Result<(), io::Error> {
        persist_across_threads()
    }

    #[test]
    fn persist_across_threads_b() -> Result<(), io::Error> {
        persist_across_threads()
    }

    #[test]
    fn persist_across_threads_c() -> Result<(), io::Error> {
        persist_across_threads()
    }

    #[test]
    fn persist_across_threads_d() -> Result<(), io::Error> {
        persist_across_threads()
    }
}
