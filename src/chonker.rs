use std::{
    cell::{RefCell, UnsafeCell},
    fs::{File, OpenOptions},
    io::{self, Write},
    marker::PhantomData,
    ops::Deref,
    path::Path,
    sync::Arc,
};

use rkyv::{
    archived_root,
    ser::{serializers::WriteSerializer, Serializer},
    AlignedVec, Archive, Serialize,
};

use parking_lot::ReentrantMutex;

use memmap::Mmap;

pub type DefaultSer<'a> = WriteSerializer<&'a mut [u8]>;

pub trait Chonkable: for<'a> Serialize<DefaultSer<'a>> {}

impl<T> Chonkable for T where T: for<'a> Serialize<DefaultSer<'a>> {}

pub struct Offset<T>(u64, PhantomData<T>);

pub struct RawOffset(u64);

impl<T> std::fmt::Debug for Offset<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Offset").field(&self.0).finish()
    }
}

impl<T> Clone for Offset<T> {
    fn clone(&self) -> Self {
        Offset::new(self.0)
    }
}

impl<T> Copy for Offset<T> {}

impl<T> Deref for Offset<T> {
    type Target = u64;

    fn deref(&self) -> &u64 {
        &self.0
    }
}

impl<T> Offset<T> {
    fn new(ofs: u64) -> Self {
        debug_assert!(ofs % std::mem::align_of::<T>() as u64 == 0);
        Offset(ofs, PhantomData)
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

/// Chonker
///
/// A hybrid memory/disk storage for an append only sequence of bytes.
#[derive(Clone, Debug, Default)]
pub struct Chonker(Arc<ChonkerInner>);

impl Chonker {
    /// Creates a new empty chonker
    pub fn new() -> Self {
        Default::default()
    }

    /// Commits a value to the chonker
    pub fn put<T>(&self, t: &T) -> Offset<T>
    where
        T: Archive + Chonkable,
    {
        self.0.put(t)
    }

    /// Gets a value previously commited to the chonker at offset `ofs`
    pub fn get<T>(&self, ofs: Offset<T>) -> &T::Archived
    where
        T: Archive,
    {
        self.0.get(ofs)
    }

    /// Persist the chonker to disk
    pub fn persist<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        self.0.persist(path)
    }
    /// Restore a chonker from disk
    pub fn restore<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        Ok(Chonker(Arc::new(ChonkerInner::restore(path)?)))
    }
}

/// Memory backend that never re-allocates
struct ChonkerInner {
    lanes: UnsafeCell<[Lane; N_LANES]>,
    written: ReentrantMutex<RefCell<u64>>,
}

impl std::fmt::Debug for ChonkerInner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ChonkerInner").finish()
    }
}

impl Default for ChonkerInner {
    fn default() -> Self {
        ChonkerInner {
            lanes: UnsafeCell::new(Default::default()),
            written: ReentrantMutex::new(RefCell::new(0)),
        }
    }
}

unsafe impl Sync for ChonkerInner {}

const fn lane_from_offset(offset: u64) -> (usize, usize) {
    const USIZE_BITS: usize = std::mem::size_of::<usize>() * 8;
    let i = offset / FIRST_CHONK_SIZE as u64 + 1;
    let lane = USIZE_BITS - i.leading_zeros() as usize - 1;
    let lane_offset =
        offset - (2u64.pow(lane as u32) - 1) * FIRST_CHONK_SIZE as u64;
    (lane, lane_offset as usize)
}

const fn lane_size_from_lane(lane: usize) -> usize {
    FIRST_CHONK_SIZE * 2usize.pow(lane as u32)
}

impl ChonkerInner {
    /// Stores a value into the chonker
    fn put<T>(&self, t: &T) -> Offset<T>
    where
        T: Archive + Chonkable,
    {
        let lock = self.written.lock();
        let mut written = lock.borrow_mut();

        let archived_size = std::mem::size_of::<T::Archived>();
        let alignment = std::mem::align_of::<T::Archived>();

        let alignment_pad = (*written % alignment as u64) as usize;

        let lanes = unsafe { &mut *self.lanes.get() };

        let (mut lane, mut lane_written) = lane_from_offset(*written);

        loop {
            let cap = lane_size_from_lane(lane);
            match &mut lanes[lane] {
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
                    let space_left = cap - lane_written - alignment_pad;
                    // No space
                    if space_left < archived_size {
                        // Take into account the padding at the end of the lane
                        *written += space_left as u64;

                        // Try writing in the next lane
                        lane += 1;
                        lane_written = 0;
                    } else {
                        // Enough room to write here
                        *written += alignment_pad as u64;

                        let offset = Offset::new(*written);

                        *written += archived_size as u64;

                        let slice = if let Some(map) = map {
                            let ofs = lane_written - map.len();
                            unsafe { ram.set_len(ofs + archived_size) };
                            &mut ram[ofs..][..archived_size]
                        } else {
                            unsafe {
                                ram.set_len(lane_written + archived_size)
                            };
                            &mut ram[lane_written..][..archived_size]
                        };
                        let mut serializer = WriteSerializer::new(slice);
                        serializer.serialize_value(t).expect("infallible");
                        return offset;
                    }
                }
            }
        }
    }

    /// Gets a value from the chonker at offset `ofs`
    fn get<T>(&self, ofs: Offset<T>) -> &T::Archived
    where
        T: Archive,
    {
        let (lane, lane_ofs) = lane_from_offset(*ofs);
        let archived_len = std::mem::size_of::<T::Archived>();

        let lanes = unsafe { &*self.lanes.get() };

        match &lanes[lane] {
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

    /// Persist the chonker to disk
    fn persist<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        // We take the write guard to make sure writes block until persistance
        // is complete.
        let _write = self.written.lock();

        for (i, lane) in
            unsafe { &mut *self.lanes.get() }.iter_mut().enumerate()
        {
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

    /// Open a chonker from disk
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

                written += map.len() as u64;

                *lane = Lane {
                    map: Some(map),
                    file: Some(file),
                    ram: None,
                };
            } else {
                break;
            }
        }

        Ok(ChonkerInner {
            lanes: UnsafeCell::new(lanes),
            written: ReentrantMutex::new(RefCell::new(written)),
        })
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use rend::LittleEndian;

    const FCS: u64 = FIRST_CHONK_SIZE as u64;

    #[test]
    fn many_raw() {
        let chonker = Chonker::default();

        const N: usize = 1024 * 64;

        let mut references = vec![];

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(chonker.put(&le));
        }

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(chonker.get(references[i]), &le);
        }
    }

    #[test]
    fn lane_math() {
        assert_eq!(lane_from_offset(0), (0, 0));
        assert_eq!(lane_from_offset(1), (0, 1));
        assert_eq!(lane_from_offset(FCS), (1, 0));
        assert_eq!(lane_from_offset(FCS + 32), (1, 32));
        assert_eq!(lane_from_offset(FCS * 2), (1, FCS as usize));
        assert_eq!(lane_from_offset(FCS * 3), (2, 0));
    }

    #[test]
    fn many_raw_persist() -> io::Result<()> {
        let chonker = Chonker::default();

        const N: usize = 1024 * 64;

        let mut references = vec![];

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(chonker.put(&le));
        }

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(chonker.get(references[i]), &le);
        }

        use tempfile::tempdir;

        let dir = tempdir()?;

        chonker.persist(dir.path())?;

        drop(chonker);

        let new_chonker = Chonker::restore(&dir)?;

        // read the same values from disk

        for i in 0..N {
            let le: LittleEndian<u32> = (i as u32).into();

            assert_eq!(new_chonker.get(references[i]), &le);
        }

        // now write some more!

        for i in N..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            references.push(new_chonker.put(&le));
        }

        // and read all back

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            let ofs = references[i];

            assert_eq!(new_chonker.get(ofs), &le);
        }

        // persist again

        new_chonker.persist(dir.path())?;

        drop(new_chonker);

        let even_newer_chonker = Chonker::restore(dir)?;

        // read all back again

        for i in 0..N * 2 {
            let le: LittleEndian<u32> = (i as u32).into();

            let ofs = references[i];

            assert_eq!(even_newer_chonker.get(ofs), &le);
        }

        Ok(())
    }
}
