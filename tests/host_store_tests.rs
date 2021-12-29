// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use microkelvin::{HostStore, Ident, Offset, Store};
use rkyv::rend::LittleEndian;
use std::io;

#[test]
fn it_works() {
    let store = HostStore::new();

    let a = LittleEndian::<i128>::new(8);

    let ident = store.put(&a);
    let res = ident.inner();

    assert_eq!(*res, a);
}

#[test]
fn lot_more() {
    let store = HostStore::new();

    let mut ids = vec![];

    for i in 0..1024 {
        ids.push(store.put(&LittleEndian::<i128>::new(i)));
    }

    for (stored, i) in ids.iter().zip(0..) {
        let comp = LittleEndian::from(i as i128);
        let got = stored.inner();
        assert_eq!(*got, comp)
    }
}

#[test]
fn many_raw_persist_and_restore() -> io::Result<()> {
    const N: usize = 1024 * 64;

    let mut references = vec![];

    use tempfile::tempdir;

    let dir = tempdir()?;

    let mut host_store = HostStore::with_file(dir.path())?;

    for i in 0..N {
        let le: LittleEndian<u32> = (i as u32).into();

        references.push(host_store.put(&le));
    }

    let le: LittleEndian<u32> = (0 as u32).into();

    assert_eq!(
        host_store.get_raw::<LittleEndian<u32>>(&references[0].ident()),
        &le
    );

    let le: LittleEndian<u32> = (65534 as u32).into();

    assert_eq!(
        host_store.get_raw::<LittleEndian<u32>>(&references[65534].ident()),
        &le
    );

    let le: LittleEndian<u32> = (65535 as u32).into();

    assert_eq!(
        host_store.get_raw::<LittleEndian<u32>>(&references[65535].ident()),
        &le
    );

    for i in 0..N {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(
            host_store.get_raw::<LittleEndian<u32>>(&references[i].ident()),
            &le
        );
    }

    for i in 0..N {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(
            host_store.get_raw::<LittleEndian<u32>>(&references[i].ident()),
            &le
        );
    }

    host_store.persist()?;

    // now write some more!

    for i in N..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        references.push(host_store.put(&le));
    }

    // and read all back

    for i in 0..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(
            host_store.get_raw::<LittleEndian<u32>>(&references[i].ident()),
            &le
        );
    }

    // read all back again

    for i in 0..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(
            host_store.get_raw::<LittleEndian<u32>>(&references[i].ident()),
            &le
        );
    }

    // persist again and restore

    host_store.persist()?;

    let host_store_restored = HostStore::with_file(dir.path())?;

    for i in 0..N * 2 {
        let le: LittleEndian<u32> = (i as u32).into();

        assert_eq!(
            host_store_restored
                .get_raw::<LittleEndian<u32>>(&references[i].ident()),
            &le
        );
    }

    Ok(())
}
