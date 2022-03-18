// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use microkelvin::{HostStore, StoreRef};
use rkyv::rend::LittleEndian;
use std::io;

#[test]
fn it_works() {
    let store = StoreRef::new(HostStore::new());

    let a = LittleEndian::<i128>::new(8);

    let stored = store.store(&a);

    let res = store.get(stored.ident());

    assert_eq!(*res, a);
}

#[test]
fn lot_more() {
    let store = StoreRef::new(HostStore::new());

    let mut ids = vec![];

    for i in 0..1024 {
        ids.push(store.store(&LittleEndian::<i128>::new(i)));
    }

    for (stored, i) in ids.iter().zip(0..) {
        let comp = LittleEndian::from(i as i128);
        let got = stored.inner();
        assert_eq!(*got, comp)
    }
}

#[test]
fn many_raw_persist_and_restore() -> Result<(), io::Error> {
    const N: usize = 1024 * 64;

    let mut references = vec![];

    use tempfile::tempdir;

    let dir = tempdir()?;

    let host_store = StoreRef::new(HostStore::with_file(dir.path())?);

    for i in 0..N {
        let le: LittleEndian<u32> = (i as u32).into();

        references.push(host_store.put(&le));
    }

    let le: LittleEndian<u32> = (0 as u32).into();

    assert_eq!(host_store.get::<LittleEndian<u32>>(&references[0]), &le);

    let le: LittleEndian<u32> = (65534 as u32).into();

    assert_eq!(host_store.get::<LittleEndian<u32>>(&references[65534]), &le);

    let le: LittleEndian<u32> = (65535 as u32).into();

    assert_eq!(host_store.get::<LittleEndian<u32>>(&references[65535]), &le);

    for i in 0..N {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(host_store.get::<LittleEndian<u32>>(&references[i]), &le);
    }

    for i in 0..N {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(host_store.get::<LittleEndian<u32>>(&references[i]), &le);
    }

    host_store.persist().unwrap();

    // now write some more!

    for i in N..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        references.push(host_store.put(&le));
    }

    // and read all back

    for i in 0..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(host_store.get::<LittleEndian<u32>>(&references[i]), &le);
    }

    // read all back again

    for i in 0..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(host_store.get::<LittleEndian<u32>>(&references[i]), &le);
    }

    // persist again and restore

    host_store.persist().unwrap();

    let host_store_restored = StoreRef::new(HostStore::with_file(dir.path())?);

    for i in 0..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(host_store_restored.get(&references[i]), &le);
    }

    Ok(())
}

#[test]
fn big_items_persist_and_restore() -> Result<(), io::Error> {

    const SZ: usize = 35176; // size is more than half of the page size
    let item1 = [1u8; SZ];
    let item2 = [2u8; SZ];

    use tempfile::tempdir;

    let dir = tempdir()?;

    let host_store = StoreRef::new(HostStore::with_file(dir.path())?);

    let ident1 = host_store.put(&item1 );
    let ident2 = host_store.put(&item2 );

    host_store.persist().unwrap();

    let host_store_restored = StoreRef::new(HostStore::with_file(dir.path())?);

    let restored1 = host_store_restored.get::<[u8;SZ]>(&ident1);
    let restored2 = host_store_restored.get::<[u8;SZ]>(&ident2);

    for b in restored1.into_iter() {
        assert_eq!(*b, 1u8)
    }
    for b in restored2.into_iter() {
        assert_eq!(*b, 2u8)
    }

    Ok(())
}
