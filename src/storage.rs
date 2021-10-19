// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::{
    borrow::{Borrow, BorrowMut},
    fs::{File, OpenOptions},
    io::{self, Write},
    marker::PhantomData,
    ops::Deref,
    path::Path,
    sync::Arc,
};

lazy_static! {
    static ref SINGLETON: Arc<RwLock<Storage>> =
        Arc::new(RwLock::new(Storage::default()));
}

use rkyv::{
    archived_root, ser::Serializer, AlignedVec, Archive, Fallible, Infallible,
    Serialize,
};

use parking_lot::RwLock;

use memmap::Mmap;

pub struct Stored<T> {
    offset: RawOffset,
    _marker: PhantomData<T>,
}

impl<T> Clone for Stored<T> {
    fn clone(&self) -> Self {
        Stored {
            offset: self.offset.clone(),
            _marker: PhantomData,
        }
    }
}

impl<T> Copy for Stored<T> {}

impl<T> Stored<T>
where
    T: Archive,
{
    pub(crate) fn new(offset: RawOffset) -> Self {
        debug_assert!(*offset % std::mem::align_of::<T>() as u64 == 0);
        Stored {
            offset,
            _marker: PhantomData,
        }
    }

    pub fn offset(self) -> u64 {
        *self.offset
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RawOffset(u64);

impl Deref for RawOffset {
    type Target = u64;

    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl RawOffset {
    fn new(u: u64) -> Self {
        RawOffset(u)
    }
}

/// Helper trait to constrain serializers used with Storage;
pub trait StorageSerializer: Serializer + Sized + BorrowMut<Storage> {}
impl<T> StorageSerializer for T where T: Serializer + Sized + BorrowMut<Storage> {}

/// Helper trait to constrain deserializers used with Storage;
pub trait PortalDeserializer: Fallible + Sized + Borrow<Portal> {}
impl<T> PortalDeserializer for T where T: Fallible + Sized + Borrow<Portal> {}

impl<T> std::fmt::Debug for Stored<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Offset").field(&self.offset).finish()
    }
}

const FIRST_CHONK_SIZE: usize = 64 * 1024;
const N_LANES: usize = 32;

#[derive(Default)]
pub struct Lane {
    ram: Option<AlignedVec>,
    #[allow(unused)]
    file: Option<File>,
    map: Option<Mmap>,
}

impl std::fmt::Debug for Lane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Lane")
            .field("ram", &self.ram.as_ref().map(|_| ()))
            .field("file", &self.file.as_ref().map(|_| ()))
            .field("map", &self.map.as_ref().map(|_| ()))
            .finish()
    }
}

/// Portal
///
/// A hybrid memory/disk storage for an append only sequence of bytes.
#[derive(Clone, Debug, Default)]
pub struct Portal;

impl Portal {
    /// Commits a value to the portal
    pub fn put<T>(t: &T) -> Stored<T>
    where
        T: Archive + Serialize<Storage>,
    {
        SINGLETON.write().put(t)
    }

    /// Gets a value previously commited to the portal at offset `ofs`
    pub fn get<'a, T>(ofs: Stored<T>) -> &'a T::Archived
    where
        T: Archive,
    {
        let read = SINGLETON.read();
        let archived: &T::Archived = read.get::<T>(ofs);
        // extend the lifetime to equal the lifetime of the `Portal`.
        // This is safe, since the reference is guaranteed to not move until the
        // process is shut down.
        let extended: &'a T::Archived =
            unsafe { std::mem::transmute(archived) };
        extended
    }

    /// Persist the portal to disk
    pub fn persist<P: AsRef<Path>>(path: P) -> io::Result<()> {
        SINGLETON.write().persist(path)
    }

    /// Restore a portal from disk
    pub fn restore<P: AsRef<Path>>(path: P) -> io::Result<()> {
        let mut write = SINGLETON.write();
        *write = Storage::restore(path)?;
        Ok(())
    }
}

/// Memory backend that never re-allocates
pub struct Storage {
    lanes: [Lane; N_LANES],
    written: usize,
}

impl Fallible for Storage {
    type Error = Infallible;
}

impl Fallible for &Portal {
    type Error = Infallible;
}

impl Serializer for Storage {
    fn pos(&self) -> usize {
        self.written
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Self::Error> {
        let (mut lane, mut lane_written) = lane_from_offset(self.written);
        let bytes_len = bytes.len();

        loop {
            let cap = lane_size_from_lane(lane);
            match &mut self.lanes[lane] {
                Lane {
                    ram: ram @ None, ..
                } => {
                    let vec = AlignedVec::with_capacity(cap);
                    *ram = Some(vec);
                }
                Lane {
                    ram: Some(ram),
                    map,
                    ..
                } => {
                    let space_left = cap - lane_written;
                    // No space
                    if space_left < bytes_len {
                        // Take into account the padding at the end of the lane
                        self.written += space_left;

                        // Try writing in the next lane
                        lane += 1;
                        lane_written = 0;
                    } else {
                        self.written += bytes_len;

                        let buffer = if let Some(map) = map {
                            let ofs = lane_written - map.len();
                            unsafe { ram.set_len(ofs + bytes_len) };
                            &mut ram[ofs..][..bytes_len]
                        } else {
                            unsafe { ram.set_len(lane_written + bytes_len) };
                            &mut ram[lane_written..][..bytes_len]
                        };

                        buffer.copy_from_slice(bytes);
                        return Ok(());
                    }
                }
            }
        }
    }
}

impl std::fmt::Debug for Storage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Storage").finish()
    }
}

impl Default for Storage {
    fn default() -> Self {
        Storage {
            lanes: Default::default(),
            written: 0,
        }
    }
}

unsafe impl Sync for Storage {}

const fn lane_from_offset(offset: usize) -> (usize, usize) {
    const USIZE_BITS: usize = std::mem::size_of::<usize>() * 8;
    let i = offset / FIRST_CHONK_SIZE + 1;
    let lane = USIZE_BITS - i.leading_zeros() as usize - 1;
    let lane_offset = offset - (2usize.pow(lane as u32) - 1) * FIRST_CHONK_SIZE;
    (lane, lane_offset)
}

const fn lane_size_from_lane(lane: usize) -> usize {
    FIRST_CHONK_SIZE * 2usize.pow(lane as u32)
}

impl Storage {
    /// Commits a value to the portal
    pub fn put<T>(&mut self, t: &T) -> Stored<T>
    where
        T: Archive + Serialize<Storage>,
    {
        let _ = self.serialize_value(t);
        let ofs = self.written - std::mem::size_of::<T::Archived>();
        Stored::new(RawOffset::new(ofs as u64))
    }

    /// Gets a value from the portal at offset `ofs`
    fn get<T>(&self, stored: Stored<T>) -> &T::Archived
    where
        T: Archive,
    {
        let ofs = stored.offset();
        let (lane, lane_ofs) = lane_from_offset(ofs as usize);
        let archived_len = std::mem::size_of::<T::Archived>();

        match &self.lanes[lane] {
            Lane {
                ram: Some(ram),
                map,
                ..
            } => {
                let slice = if let Some(map) = map {
                    let map_len = map.len();
                    if lane_ofs < map_len {
                        &map[lane_ofs..][..archived_len]
                    } else {
                        &ram[lane_ofs - map_len..][..archived_len]
                    }
                } else {
                    &ram[lane_ofs..][..archived_len]
                };
                unsafe { archived_root::<T>(slice) }
            }
            Lane {
                map: Some(map),
                ram: None,
                ..
            } => {
                let slice = &map[lane_ofs..][..archived_len];
                unsafe { archived_root::<T>(slice) }
            }
            e @ _ => panic!("Invalid offset {:?}", e),
        }
    }

    /// Persist the portal to disk
    fn persist<P: AsRef<Path>>(&mut self, path: P) -> io::Result<()> {
        for (i, lane) in self.lanes.iter_mut().enumerate() {
            match lane {
                Lane { ram: None, .. } => {
                    // no-op
                }
                Lane {
                    ram: Some(ram),
                    file: file_slot @ None,
                    ..
                } => {
                    let path = path.as_ref().join(format!("lane_{}", i));
                    let mut file = OpenOptions::new()
                        .append(true)
                        .create(true)
                        .open(&path)?;
                    file.write_all(ram.as_slice())?;
                    file.flush()?;
                    *file_slot = Some(file);
                }
                Lane {
                    ram: Some(ram),
                    file: Some(file),
                    ..
                } => {
                    file.write_all(ram.as_slice())?;
                    file.flush()?;
                    // already a file.
                }
            }
        }
        Ok(())
    }

    /// Open a portal from disk
    fn restore<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        // We take the write guard to make sure writes block until persistance
        // is complete.

        let mut lanes: [Lane; N_LANES] = Default::default();

        let mut written = 0;

        for (i, lane) in lanes.iter_mut().enumerate() {
            let path = path.as_ref().join(format!("lane_{}", i));

            if path.exists() {
                let file = OpenOptions::new()
                    .append(true)
                    .read(true)
                    .create(false)
                    .open(&path)?;

                let map = unsafe { Mmap::map(&file)? };

                written += map.len();

                *lane = Lane {
                    map: Some(map),
                    file: Some(file),
                    ram: None,
                };
            } else {
                break;
            }
        }

        Ok(Storage {
            lanes: lanes,
            written,
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rend::LittleEndian;

    #[test]
    fn many_raw() {
        const N: usize = 1024 * 64;

        let mut references = vec![];

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(Portal::put(&le));
        }

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(Portal::get(references[i]), &le);
        }
    }

    #[test]
    fn lane_math() {
        const FCS: usize = FIRST_CHONK_SIZE;

        assert_eq!(lane_from_offset(0), (0, 0));
        assert_eq!(lane_from_offset(1), (0, 1));
        assert_eq!(lane_from_offset(FCS), (1, 0));
        assert_eq!(lane_from_offset(FCS + 32), (1, 32));
        assert_eq!(lane_from_offset(FCS * 2), (1, FCS as usize));
        assert_eq!(lane_from_offset(FCS * 3), (2, 0));
    }

    #[test]
    fn many_raw_persist() -> io::Result<()> {
        const N: usize = 1024 * 64;

        let mut references = vec![];

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(Portal::put(&le));
        }

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(Portal::get(references[i]), &le);
        }

        use tempfile::tempdir;

        let dir = tempdir()?;

        Portal::persist(dir.path())?;

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(Portal::get(references[i]), &le);
        }

        // now write some more!

        for i in N..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(Portal::put(&le));
        }

        // and read all back

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(Portal::get(references[i]), &le);
        }

        // read all back again

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(Portal::get(references[i]), &le);
        }

        Ok(())
    }
}
