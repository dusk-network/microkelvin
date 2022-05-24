// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::BorrowMut;
use core::cell::RefCell;

use alloc::rc::Rc;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Fallible, Serialize};

use crate::storage::{Ident, StoreProvider, Stored, UnwrapInfallible};
use crate::tower::{WellArchived, WellFormed};
use crate::wrappers::MaybeStored;
use crate::{ARef, Annotation, Compound, StoreSerializer};

#[derive(Clone, Debug)]
/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub enum Link<C, A> {
    /// A Link to a node in memory
    Memory {
        /// the underlying rc
        rc: Rc<C>,
        /// an optional annotation
        annotation: RefCell<Option<A>>,
    },
    /// A Link to a stored node
    Stored {
        /// archived at offset
        stored: Stored<C>,
        /// the final annotation
        a: A,
    },
}

#[derive(CheckBytes)]
/// The archived version of a link, contains an identifier and an annotation
pub struct ArchivedLink<C, A>(Ident<C>, A);

impl<C, A> ArchivedLink<C, A> {
    /// Get a reference to the link annotation
    pub fn annotation(&self) -> &A {
        &self.1
    }

    /// Get a reference to the link id     
    pub fn ident<'a>(&self) -> &Ident<C> {
        &self.0
    }
}

impl<C, A> Archive for Link<C, A> {
    type Archived = ArchivedLink<C, A>;
    type Resolver = (Ident<C>, A);

    unsafe fn resolve(
        &self,
        _pos: usize,
        resolver: Self::Resolver,
        out: *mut <Self as Archive>::Archived,
    ) {
        *out = ArchivedLink(resolver.0, resolver.1)
    }
}

impl<C, A, D> Deserialize<Link<C, A>, D> for ArchivedLink<C, A>
where
    A: Clone,
    D: StoreProvider,
{
    fn deserialize(&self, de: &mut D) -> Result<Link<C, A>, D::Error> {
        Ok(Link::Stored {
            stored: Stored::new(de.store().clone(), self.0.clone()),
            a: self.1.clone(),
        })
    }
}

impl<C, A, S> Serialize<S> for Link<C, A>
where
    C: Compound<A> + WellFormed,
    C::Leaf: WellFormed,
    A: Annotation<C::Leaf>,
    S: BorrowMut<StoreSerializer> + Fallible,
{
    fn serialize(
        &self,
        ser: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        match self {
            Link::Memory { rc, annotation } => {
                let borrow = annotation.borrow();
                let a = if let Some(a) = &*borrow {
                    a.clone()
                } else {
                    let a = A::from_node(&**rc);
                    drop(borrow);
                    *annotation.borrow_mut() = Some(a.clone());
                    a
                };
                let to_serialize = &(**rc);

                let store_ser = ser.borrow_mut();
                store_ser.serialize(to_serialize);
                let id = Ident::new(store_ser.commit());
                Ok((id, a))
            }
            Link::Stored { stored, a } => {
                Ok((stored.ident().clone(), a.clone()))
            }
        }
    }
}

impl<C, A> Default for Link<C, A>
where
    C: Default,
{
    fn default() -> Self {
        Link::Memory {
            rc: Rc::new(C::default()),
            annotation: RefCell::new(None),
        }
    }
}

impl<C, A> Link<C, A> {
    /// Create a new link
    pub fn new(compound: C) -> Self {
        Link::Memory {
            rc: Rc::new(compound),
            annotation: RefCell::new(None),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> ARef<A>
    where
        C: Compound<A> + WellFormed,
        C::Leaf: WellFormed,
        A: Annotation<C::Leaf>,
    {
        match self {
            Link::Memory { rc, annotation, .. } => {
                let borrow = annotation.borrow();
                if let Some(_) = *borrow {
                    ARef::Referenced(borrow)
                } else {
                    drop(borrow);
                    *annotation.borrow_mut() = Some(A::from_node(&**rc));
                    self.annotation()
                }
            }
            Link::Stored { a, .. } => ARef::Borrowed(a),
        }
    }

    /// Unwraps the underlying value, clones or deserializes it
    #[deprecated(since = "like, forever")]
    pub fn unlink(self) -> C
    where
        C: Compound<A> + WellFormed,
        C::Archived: WellArchived<C>,
    {
        self.into_inner()
    }

    /// Unwraps the underlying value, clones or deserializes it
    pub fn into_inner(self) -> C
    where
        C: Compound<A> + WellFormed,
        C::Archived: WellArchived<C>,
    {
        match self {
            Link::Memory { rc, .. } => match Rc::try_unwrap(rc) {
                Ok(c) => c,
                Err(rc) => (*rc).clone(),
            },
            Link::Stored { stored, .. } => {
                let inner: &C::Archived = stored.inner();
                let de: C = inner
                    .deserialize(&mut stored.store().clone())
                    .unwrap_infallible();
                de
            }
        }
    }

    /// Returns a reference to the inner node, possibly in its stored form
    pub fn inner(&self) -> MaybeStored<C>
    where
        C: Archive,
    {
        match self {
            Link::Memory { rc, .. } => MaybeStored::Memory(&(*rc)),
            Link::Stored { stored, .. } => MaybeStored::Stored(stored),
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    pub fn inner_mut(&mut self) -> &mut C
    where
        C: WellFormed,
        C::Archived: WellArchived<C>,
    {
        match self {
            Link::Memory { rc, annotation } => {
                // clear annotation
                annotation.borrow_mut().take();
                return Rc::make_mut(rc);
            }
            Link::Stored { stored, .. } => {
                let mut store = stored.store().clone();
                let inner = stored.inner();
                let c: C = inner.deserialize(&mut store).unwrap_infallible();

                *self = Link::Memory {
                    rc: Rc::new(c),
                    annotation: RefCell::new(None),
                };
                self.inner_mut()
            }
        }
    }
}
