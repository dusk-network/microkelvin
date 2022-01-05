use alloc::sync::Arc;
use core::convert::Infallible;

use bytecheck::CheckBytes;
use rkyv::ser::Serializer;
use rkyv::validation::validators::DefaultValidator;
use rkyv::{check_archived_root, Archive, Fallible, Serialize};

use crate::{Ident, Store, StoreProvider, StoreSerializer, Stored};

// TODO: Create alternative for no_alloc
/// A clonable reference to a store
pub struct StoreRef<I> {
    inner: Arc<dyn Store<Identifier = I> + Send + Sync>,
}

impl<I> StoreRef<I> {
    /// Creates a new StoreReference
    pub fn new<S: 'static + Store<Identifier = I> + Send + Sync>(
        store: S,
    ) -> StoreRef<I> {
        StoreRef {
            inner: Arc::new(store),
        }
    }
}

impl<I> StoreRef<I> {
    /// Put a value into storage
    pub fn put<T>(&self, t: &T) -> Stored<T, I>
    where
        T: for<'any> Serialize<StoreSerializer<'any, I>>,
    {
        let buffer = self.inner.write();
        let mut ser = StoreSerializer::new(self.clone(), buffer);

        match ser.serialize_value(t) {
            Ok(size) => Stored::new(
                self.clone(),
                Ident::new(self.inner.commit(ser.consume(), size)),
            ),
            Err(_e) => {
                todo!("Create new page and try again")
            }
        }
    }

    /// Gets a reference to an archived value
    pub fn get<T>(&self, ident: &Ident<T, I>) -> &T::Archived
    where
        T: Archive,
        T::Archived: for<'a> CheckBytes<DefaultValidator<'a>>,
    {
        let buffer = self.inner.get(ident.erase());

        let root = check_archived_root::<T>(buffer).unwrap();
        root
    }

    /// Persist the store to underlying storage.
    pub fn persist(&self) -> Result<(), ()> {
        self.inner.persist()
    }
}

impl<I> StoreProvider<I> for StoreRef<I> {
    fn store(&self) -> &StoreRef<I> {
        self
    }
}

impl<I> Clone for StoreRef<I> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

impl<I> Fallible for StoreRef<I> {
    type Error = Infallible;
}
