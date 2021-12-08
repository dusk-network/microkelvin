// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use core::borrow::Borrow;
use core::cell::RefCell;

use alloc::rc::Rc;

use owning_ref::OwningRef;
use rkyv::Fallible;
use rkyv::{Archive, Deserialize, Serialize};

use crate::storage::{Ident, Storage, Store, Stored, UnwrapInfallible};
use crate::wrappers::MaybeStored;
use crate::{ARef, Annotation, Compound};

#[derive(Clone)]
/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub enum Link<C, A, S>
where
    S: Store,
{
    /// A Link to a node in memory
    Memory {
        /// the underlying rc
        rc: Rc<C>,
        /// an optional annotation
        annotation: RefCell<Option<A>>,
    },
    /// A Link to an archived node
    Archived {
        /// archived at offset
        stored: Stored<C, S>,
        /// the final annotation
        a: A,
    },
}

/// The archived version of a link, contains an identifier and an annotation
pub struct ArchivedLink<C, A, S>(Ident<S::Identifier, C>, A)
where
    S: Store;

impl<C, A, S> ArchivedLink<C, A, S>
where
    S: Store,
{
    /// Get a reference to the link annotation
    pub fn annotation(&self) -> &A {
        &self.1
    }

    /// Get a reference to the link id     
    pub fn ident<'a>(&self) -> &Ident<S::Identifier, C> {
        &self.0
    }
}

impl<C, A, S> Archive for Link<C, A, S>
where
    S: Store,
{
    type Archived = ArchivedLink<C, A, S>;
    type Resolver = (Ident<S::Identifier, C>, A);

    unsafe fn resolve(
        &self,
        _pos: usize,
        resolver: Self::Resolver,
        out: *mut <Self as Archive>::Archived,
    ) {
        *out = ArchivedLink(resolver.0, resolver.1)
    }
}

impl<C, A, S, S2> Deserialize<Link<C, A, S>, S2> for ArchivedLink<C, A, S>
where
    S: Store,
    S2: Store,
    for<'a> &'a mut S2: Borrow<S>,
    A: Clone,
{
    fn deserialize(
        &self,
        store: &mut S2,
    ) -> Result<Link<C, A, S>, <S as Fallible>::Error> {
        let local_store: &S = store.borrow();
        Ok(Link::Archived {
            stored: Stored::new(local_store.clone(), self.0),
            a: self.1.clone(),
        })
    }
}

impl<C, A, S> Serialize<S::Storage> for Link<C, A, S>
where
    C: Compound<A, S> + Serialize<S::Storage>,
    A: Clone + Annotation<C::Leaf>,
    S: Store,
{
    fn serialize(
        &self,
        ser: &mut S::Storage,
    ) -> Result<Self::Resolver, S::Error> {
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
                let to_insert = &(**rc);
                let ident = Ident::new(ser.put(to_insert));
                Ok((ident, a))
            }
            Link::Archived { .. } => {
                unreachable!("FIXME motivate why this does not happen")
            }
        }
    }
}

impl<C, A, S> Default for Link<C, A, S>
where
    C: Default,
    S: Store,
{
    fn default() -> Self {
        Link::Memory {
            rc: Rc::new(C::default()),
            annotation: RefCell::new(None),
        }
    }
}

impl<C, A, S> Link<C, A, S>
where
    S: Store,
{
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
        C: Compound<A, S>,
        C::Leaf: Archive,
        A: Annotation<C::Leaf>,
    {
        match self {
            Link::Memory { rc, annotation, .. } => {
                let borrow = annotation.borrow();
                if let Some(_) = *borrow {
                    ARef::Referenced(OwningRef::new(borrow).map(|brw| {
                        if let Some(a) = &*brw {
                            a
                        } else {
                            unreachable!()
                        }
                    }))
                } else {
                    drop(borrow);
                    *annotation.borrow_mut() = Some(A::from_node(&**rc));
                    self.annotation()
                }
            }
            Link::Archived { a, .. } => ARef::Borrowed(a),
        }
    }

    /// Unwraps the underlying value, clones or deserializes it
    pub fn unlink(self) -> C
    where
        C: Compound<A, S> + Clone,
        C::Archived: Deserialize<C, S>,
    {
        match self {
            Link::Memory { rc, .. } => match Rc::try_unwrap(rc) {
                Ok(c) => c,
                Err(rc) => (&*rc).clone(),
            },
            Link::Archived { stored, .. } => {
                let inner: &C::Archived = stored.inner();
                let de: C = inner
                    .deserialize(&mut stored.store().clone())
                    .unwrap_infallible();
                de
            }
        }
    }

    /// Returns a reference to the inner node, possibly in its stored form
    pub fn inner<'a>(&'a self) -> MaybeStored<'a, C, S>
    where
        C: Archive,
    {
        match self {
            Link::Memory { rc, .. } => MaybeStored::Memory(&(*rc)),
            Link::Archived { stored, .. } => MaybeStored::Stored(stored),
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    pub fn inner_mut(&mut self) -> &mut C
    where
        C: Archive + Clone,
        C::Archived: Deserialize<C, S>,
    {
        match self {
            Link::Memory { rc, annotation } => {
                // clear annotation
                annotation.borrow_mut().take();
                return Rc::make_mut(rc);
            }
            Link::Archived { stored, .. } => {
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
