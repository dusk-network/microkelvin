use core::convert::Infallible;
use core::hint::unreachable_unchecked;
use core::marker::PhantomData;
use std::sync::Arc;

use rkyv::{ser::Serializer, Archive, Fallible, Serialize};

use parking_lot::RwLock;

mod vec_storage;
pub use vec_storage::PageStorage;

use crate::{
    Annotation, ArchivedCompound, Branch, Compound, MaybeArchived, Walker,
};

pub struct Ident<I, T> {
    id: I,
    _marker: PhantomData<T>,
}

impl<I, T> Clone for Ident<I, T>
where
    I: Copy,
{
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            _marker: PhantomData,
        }
    }
}

impl<I, T> Copy for Ident<I, T> where I: Copy {}

impl<I, T> Ident<I, T> {
    /// Creates a typed identifier
    pub(crate) fn new(id: I) -> Self {
        Ident {
            id,
            _marker: PhantomData,
        }
    }

    /// Returns an untyped identifier
    pub fn erase(self) -> I {
        self.id
    }
}

/// Stored is a reference to a value stored, along with the backing store
#[derive(Clone)]
pub struct Stored<S, T>
where
    S: Store,
{
    store: S,
    ident: Ident<S::Identifier, T>,
}

unsafe impl<S, T> Send for Stored<S, T> where S: Store + Send {}
unsafe impl<S, T> Sync for Stored<S, T> where S: Store + Sync {}

impl<S, T> Stored<S, T>
where
    S: Store,
{
    pub(crate) fn new(store: S, ident: Ident<S::Identifier, T>) -> Self {
        Stored { store, ident }
    }

    pub fn store(&self) -> &S {
        &self.store
    }

    pub fn ident(&self) -> &Ident<S::Identifier, T> {
        &self.ident
    }

    pub fn inner(&self) -> &T::Archived
    where
        T: Archive,
    {
        self.store.get_raw(&self.ident)
    }

    pub fn walk<W, A>(&self, walker: W) -> Option<Branch<S, T, A>>
    where
        S: Store,
        T: Compound<S, A>,
        T::Archived: ArchivedCompound<S, T, A>,
        T::Leaf: Archive,
        A: Annotation<T::Leaf>,
        W: Walker<S, T, A>,
    {
        let inner = self.inner();
        Branch::walk_with_store(
            MaybeArchived::Archived(inner),
            walker,
            self.store().clone(),
        )
    }
}

/// A type that works as a handle to a `Storage` backend.
pub trait Store: Clone + Fallible<Error = core::convert::Infallible> {
    /// The identifier used for refering to stored values
    type Identifier: Copy;
    /// The underlying storage
    type Storage: Storage<Self::Identifier>;

    /// Put a value into storage, and get a representative token back
    fn put<T>(&self, t: &T) -> Stored<Self, T>
    where
        T: Serialize<Self::Storage>;

    /// Gets a reference to an archived value
    fn get_raw<'a, T>(
        &'a self,
        ident: &Ident<Self::Identifier, T>,
    ) -> &'a T::Archived
    where
        T: Archive;
}

/// Store that utilises a reference-counted PageStorage
#[derive(Clone)]
pub struct HostStore {
    inner: Arc<RwLock<PageStorage>>,
}

impl HostStore {
    /// Creates a new LocalStore
    pub fn new() -> Self {
        HostStore {
            inner: Arc::new(RwLock::new(PageStorage::new())),
        }
    }
}

impl Fallible for HostStore {
    type Error = Infallible;
}

impl Store for HostStore {
    type Storage = PageStorage;
    type Identifier = vec_storage::Offset;

    fn put<T>(&self, t: &T) -> Stored<Self, T>
    where
        T: Serialize<Self::Storage>,
    {
        Stored::new(self.clone(), Ident::new(self.inner.write().put::<T>(t)))
    }

    fn get_raw<'a, T>(
        &'a self,
        id: &Ident<Self::Identifier, T>,
    ) -> &'a T::Archived
    where
        T: Archive,
    {
        let guard = self.inner.read();
        let reference = guard.get::<T>(&id.erase());
        let extended: &'a T::Archived =
            unsafe { core::mem::transmute(reference) };
        extended
    }
}

/// The main trait for providing storage backends to use with `microkelvin`
pub trait Storage<I>:
    Serializer + Fallible<Error = std::convert::Infallible>
{
    /// Write a value into the storage, returns a representation
    fn put<T>(&mut self, t: &T) -> I
    where
        T: Serialize<Self>;

    /// Gets a value from the store
    fn get<T>(&self, id: &I) -> &T::Archived
    where
        T: Archive;
}

pub trait UnwrapInfallible<T> {
    fn unwrap_infallible(self) -> T;
}

impl<T> UnwrapInfallible<T> for Result<T, core::convert::Infallible> {
    fn unwrap_infallible(self) -> T {
        match self {
            Ok(t) => t,
            Err(_) => unsafe {
                // safe, since the error type cannot be instantiated
                unreachable_unchecked()
            },
        }
    }
}
