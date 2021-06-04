// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::hash::Hasher;
use std::io;
use std::sync::Arc;
use std::{
    collections::hash_map::{DefaultHasher, Entry, HashMap},
    hash::Hash,
};

mod disk;
mod test;

use crate::Child;
use canonical::{Canon, CanonError, Id};
use lazy_static::lazy_static;
use parking_lot::{RwLock, RwLockWriteGuard};

pub use disk::DiskBackend;
pub use test::TestBackend;

use crate::{Annotation, Compound, GenericTree};
pub(crate) struct WrappedBackend(Arc<RwLock<dyn Backend>>);

impl WrappedBackend {
    fn new<B: 'static + Backend>(backend: B) -> Self {
        WrappedBackend(Arc::new(RwLock::new(backend)))
    }

    pub fn get(&self, id: &Id) -> Result<GenericTree, PersistError> {
        self.0.read().get(id)
    }

    fn persist<C: Compound<A>, A>(
        &self,
        tree: &C,
    ) -> Result<PersistedId, PersistError>
    where
        C::Leaf: Canon,
        A: Annotation<C::Leaf>,
    {
        Self::persist_inner(&mut self.0.write(), tree)
    }

    pub fn persist_inner<C: Compound<A>, A>(
        backend: &mut RwLockWriteGuard<dyn Backend>,
        tree: &C,
    ) -> Result<PersistedId, PersistError>
    where
        C::Leaf: Canon,
        A: Annotation<C::Leaf>,
    {
        let generic = tree.generic();
        let id = Id::new(&generic);

        if let Some(bytes) = id.take_bytes()? {
            match backend.put(&id, &bytes[..])? {
                PutResult::Written => {
                    // Recursively store the children if not already in backend
                    for i in 0.. {
                        match tree.child(i) {
                            Child::Node(node) => {
                                Self::persist_inner(
                                    backend,
                                    &*node.compound()?,
                                )?;
                            }
                            Child::EndOfNode => break,
                            _ => (),
                        }
                    }
                }
                PutResult::AlreadyPresent => (),
            }
        }

        Ok(PersistedId(id))
    }
}

lazy_static! {
    static ref BACKENDS: Arc<RwLock<HashMap<u64, WrappedBackend>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

/// A backend constructor
pub struct BackendCtor<B> {
    ctor: fn() -> B,
    id: u64,
}

impl<B> BackendCtor<B> {
    /// Create a new constructor from a function/closure
    pub fn new(ctor: fn() -> B) -> Self {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&ctor, &mut hasher);
        let id = hasher.finish();
        BackendCtor { ctor, id }
    }
}

/// Id of a persisted GenericTree
pub struct PersistedId(Id);

impl PersistedId {
    /// Restore a GenericTree from a persistance backend.
    pub fn restore(&self) -> Result<GenericTree, PersistError> {
        Persistance::get(&self.0)
    }
}

/// The singleton interface to the persistance layer
pub struct Persistance;

impl Persistance {
    /// Persist the given Compound to a backend
    pub fn persist<C, A, B>(
        ctor: &BackendCtor<B>,
        c: &C,
    ) -> Result<PersistedId, PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Annotation<C::Leaf>,
        B: 'static + Backend,
    {
        let mut backends = BACKENDS.write();
        let entry = backends.entry(ctor.id);

        match entry {
            Entry::Occupied(mut occupied) => occupied.get_mut().persist(c),
            Entry::Vacant(vacant) => {
                let backend = (ctor.ctor)();
                vacant.insert(WrappedBackend::new(backend)).persist(c)
            }
        }
    }

    /// Get a generic tree from the backend.
    pub fn get(id: &Id) -> Result<GenericTree, PersistError> {
        // First try reifying from local store/inlined data
        if let Ok(tree) = id.reify() {
            return Ok(tree);
        }

        let backends = BACKENDS.read();

        for (_, backend) in backends.iter() {
            if let Ok(tree) = backend.get(id) {
                return Ok(tree);
            }
        }
        Err(CanonError::NotFound.into())
    }

    /// Returns a constructor for a temporary test backend
    pub fn test_backend_ctor() -> BackendCtor<TestBackend> {
        BackendCtor::new(|| {
            let dir = tempfile::tempdir().unwrap();
            let b = DiskBackend::new(dir.path()).unwrap();
            TestBackend(b, dir)
        })
    }
}

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend: Send + Sync {
    /// Get get a generic tree stored in the backend from an `Id`
    fn get(&self, id: &Id) -> Result<GenericTree, PersistError>;

    /// Write encoded bytes with a corresponding `Id` into the backend
    fn put(&mut self, id: &Id, bytes: &[u8])
        -> Result<PutResult, PersistError>;
}

/// An error that can appear when persisting structures to disk
#[derive(Debug)]
pub enum PersistError {
    /// An io-error occured while persisting
    Io(io::Error),
    /// A CanonError occured while persisting
    Canon(CanonError),
}

impl From<io::Error> for PersistError {
    fn from(e: io::Error) -> Self {
        PersistError::Io(e)
    }
}

impl From<CanonError> for PersistError {
    fn from(e: CanonError) -> Self {
        PersistError::Canon(e)
    }
}

/// Type to indicate if the backend already contained the value to write
pub enum PutResult {
    /// The bytes were written to the backend
    Written,
    /// The bytes were already present in the backend
    AlreadyPresent,
}
