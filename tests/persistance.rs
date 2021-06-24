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
    use microkelvin::{
        BackendCtor, Compound, DiskBackend, Keyed, PersistError, Persistance,
    };

    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time;

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

    static INIT_COUNTER: AtomicUsize = AtomicUsize::new(0);

    fn testbackend() -> BackendCtor<DiskBackend> {
        BackendCtor::new(|| {
            INIT_COUNTER.fetch_add(1, Ordering::SeqCst);
            DiskBackend::ephemeral()
        })
    }

    #[test]
    fn persist_a() -> Result<(), PersistError> {
        let n: u64 = 16;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.push(i);
        }

        let persisted = Persistance::persist(&testbackend(), &list)?;

        let restored_generic = persisted.restore()?;

        let mut restored: LinkedList<u64, ()> =
            LinkedList::from_generic(&restored_generic)?;

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

    // Identical to persist_a, to test concurrency

    #[test]
    fn persist_b() -> Result<(), PersistError> {
        let n: u64 = 16;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.push(i);
        }

        let persisted = Persistance::persist(&testbackend(), &list)?;

        let restored_generic = persisted.restore()?;

        let mut restored: LinkedList<u64, ()> =
            LinkedList::from_generic(&restored_generic)?;

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
    fn persist_across_threads() -> Result<(), PersistError> {
        let n: u64 = 16;

        let mut list = LinkedList::<_, ()>::new();

        for i in 0..n {
            list.push(i);
        }

        let persisted = Persistance::persist(&testbackend(), &list)?;

        // it should now be available from other threads

        std::thread::spawn(move || {
            let restored_generic = persisted.restore()?;

            let mut restored: LinkedList<u64, ()> =
                LinkedList::from_generic(&restored_generic)?;

            for i in 0..n {
                assert_eq!(restored.pop()?, Some(n - i - 1));
            }

            Ok(()) as Result<(), PersistError>
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
    fn persist_create_once() {
        while INIT_COUNTER.load(Ordering::SeqCst) == 0 {}

        for _ in 0..128 {
            assert_eq!(INIT_COUNTER.load(Ordering::SeqCst), 1);

            thread::sleep(time::Duration::from_millis(1));

            assert_eq!(INIT_COUNTER.load(Ordering::SeqCst), 1);
        }
    }
}
