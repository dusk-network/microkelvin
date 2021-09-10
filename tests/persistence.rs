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

    use microkelvin::{DiskBackend, Keyed, Portal, Putable};

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
        let n: u64 = 16;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.push(i);
        }

        let portal = Portal::new(DiskBackend::ephemeral()?);

        let id = list.put(portal)?;
        let mut restored = id.reify()?;

        // first empty the original

        for i in 0..n {
            assert_eq!(list.pop()?, Some(n - i - 1));
        }

        // then the restored copy

        for i in 0..n {
            assert_eq!(restored.pop()?, Some(n - i - 1));
        }

        Ok(())
    }

    #[test]
    fn persist_a() -> Result<(), Error> {
        persist()
    }

    #[test]
    fn persist_b() -> Result<(), Error> {
        persist()
    }

    #[test]
    fn persist_c() -> Result<(), Error> {
        persist()
    }

    #[test]
    fn persist_d() -> Result<(), Error> {
        persist()
    }

    fn persist_across_threads() -> Result<(), Error> {
        let portal = Portal::new(DiskBackend::ephemeral()?);

        let n: u64 = 16;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.push(i);
        }

        let persisted = list.put(portal)?;

        // it should now be available from other threads

        std::thread::spawn(move || {
            let mut restored = persisted.reify()?;

            for i in 0..n {
                assert_eq!(restored.pop()?, Some(n - i - 1));
            }

            Ok(()) as Result<(), Error>
        })
        .join()
        .expect("thread to join cleanly")?;

        // then empty the original

        for i in 0..n {
            assert_eq!(list.pop()?, Some(n - i - 1));
        }

        Ok(())
    }

    #[test]
    fn persist_across_threads_a() -> Result<(), Error> {
        persist_across_threads()
    }

    #[test]
    fn persist_across_threads_b() -> Result<(), Error> {
        persist_across_threads()
    }

    #[test]
    fn persist_across_threads_c() -> Result<(), Error> {
        persist_across_threads()
    }

    #[test]
    fn persist_across_threads_d() -> Result<(), Error> {
        persist_across_threads()
    }
}
