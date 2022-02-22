// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::error::Error;
use std::hash::Hasher;
use std::io;
use std::sync::Arc;
use std::{
    collections::hash_map::{DefaultHasher, Entry, HashMap},
    hash::Hash,
};

mod disk;

use canonical::{Canon, CanonError, EncodeToVec, Id, IdHash, Source};
use canonical_derive::Canon;

use lazy_static::lazy_static;
use parking_lot::RwLock;

pub use disk::DiskBackend;

use crate::{Annotation, Compound};

#[derive(Clone)]
pub struct WrappedBackend(Arc<dyn Backend>);

impl WrappedBackend {
    fn new<B: 'static + Backend>(backend: B) -> Self {
        WrappedBackend(Arc::new(backend))
    }

    pub fn get<C: Canon>(&self, id: &Id) -> Result<C, PersistError> {
        let res = self.0.get(&id.hash())?;
        let mut source = Source::new(&res);
        C::decode(&mut source).map_err(|e| PersistError::from(e))
    }

    pub fn put(&self, bytes: &[u8]) -> Result<Id, PersistError> {
        self.0.put(bytes)
    }

    pub fn persist<C, A>(&self, tree: &C) -> Result<PersistedId, PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Annotation<C::Leaf>,
    {
        let buf = tree.encode_to_vec();
        Ok(PersistedId(self.put(&buf)?))
    }
}

lazy_static! {
    static ref BACKENDS: Arc<RwLock<HashMap<u64, WrappedBackend>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

/// A backend constructor
pub struct BackendCtor<B> {
    ctor: fn() -> Result<B, PersistError>,
    id: u64,
}

impl<B> BackendCtor<B> {
    /// Create a new constructor from a function/closure
    pub fn new(ctor: fn() -> Result<B, PersistError>) -> Self {
        let mut hasher = DefaultHasher::new();
        Hash::hash(&ctor, &mut hasher);
        let id = hasher.finish();
        BackendCtor { ctor, id }
    }
}

/// Id of a persisted GenericTree
#[derive(Canon, Clone, Copy, Debug)]
pub struct PersistedId(Id);

impl PersistedId {
    /// Restore a GenericTree from a persistence backend.
    pub fn restore<C>(&self) -> Result<C, PersistError>
    where
        C: Canon,
    {
        Persistence::get(&self.0)
    }

    /// Returns the wrapped Id
    pub fn into_inner(self) -> Id {
        self.0
    }
}

/// The singleton interface to the persistence layer
pub struct Persistence;

impl Persistence {
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
        Self::with_backend(ctor, |backend| backend.persist(c))
    }

    /// Puts raw bytes in the backend
    pub fn put(bytes: &[u8]) -> Result<Id, PersistError> {
        let backends = BACKENDS.read();
        match (*backends).iter().next() {
            Some((_, backend)) => backend.put(bytes),
            None => return Err(PersistError::BackendUnavailable),
        }
    }

    /// Persist the given Compound to the default backend
    pub fn persist_default<C, A>(c: &C) -> Result<PersistedId, PersistError>
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Annotation<C::Leaf>,
    {
        let mut all = vec![];

        {
            let read = BACKENDS.read();
            for (_, b) in read.iter() {
                all.push(b.clone())
            }
        }

        let mut iter = all.iter();

        let id;

        match iter.next() {
            Some(b) => id = b.persist(c)?,
            None => return Err(PersistError::BackendUnavailable),
        };

        for b in iter {
            b.persist(c)?;
        }

        Ok(id)
    }

    /// Performs an operation with reference to a backend
    ///
    /// Also used for initializing backends on startup.
    pub fn with_backend<B, F, R>(
        ctor: &BackendCtor<B>,
        closure: F,
    ) -> Result<R, PersistError>
    where
        F: Fn(&mut WrappedBackend) -> Result<R, PersistError>,
        B: 'static + Backend,
    {
        let mut backend = {
            let mut backends = BACKENDS.write();
            match backends.entry(ctor.id) {
                Entry::Occupied(mut occupied) => occupied.get_mut().clone(),
                Entry::Vacant(vacant) => {
                    let backend = (ctor.ctor)()?;
                    vacant.insert(WrappedBackend::new(backend)).clone()
                }
            }
        };
        closure(&mut backend)
    }

    /// Get a generic tree from the backend.
    pub fn get<C>(id: &Id) -> Result<C, PersistError>
    where
        C: Canon,
    {
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
}

/// The trait defining a disk or network backend for microkelvin structures.
pub trait Backend: Send + Sync {
    /// Get get a generic tree stored in the backend from an `Id`
    fn get(&self, id: &IdHash) -> Result<Vec<u8>, PersistError>;

    /// Write encoded bytes with a corresponding `Id` into the backend
    fn put(&self, bytes: &[u8]) -> Result<Id, PersistError>;
}

/// An error that can happen when persisting structures to disk
#[derive(Debug)]
pub enum PersistError {
    /// An io-error occured while persisting
    Io(io::Error),
    /// No backend found
    BackendUnavailable,
    /// A CanonError occured while persisting
    Canon(CanonError),
    /// Other backend specific error
    Other(Box<dyn Error + Send>),
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
