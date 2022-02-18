// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::sync::Arc;
use core::mem;
use core::ops::{Deref, DerefMut};
use spin::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use canonical::{Canon, CanonError, Id, Sink, Source};

#[cfg(feature = "persistence")]
use crate::persist::{PersistError, Persistence};

use crate::generic::GenericTree;
use crate::{Annotation, Compound};

#[derive(Debug, Clone)]
enum LinkInner<C, A> {
    Placeholder,
    C(Arc<C>),
    Ca(Arc<C>, A),
    Ia(Id, A),
    Ic(Id, Arc<C>),
    #[allow(unused)]
    Ica(Id, Arc<C>, A),
}

/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub struct Link<C, A> {
    inner: RwLock<LinkInner<C, A>>,
}

impl<C, A> Clone for Link<C, A>
where
    C: Clone,
    A: Clone,
{
    fn clone(&self) -> Self {
        Self {
            inner: RwLock::new(self.inner.read().clone()),
        }
    }
}

impl<C, A> Default for Link<C, A>
where
    C: Default,
{
    fn default() -> Self {
        Link {
            inner: RwLock::new(LinkInner::C(Arc::new(C::default()))),
        }
    }
}

impl<C: core::fmt::Debug, A: core::fmt::Debug> core::fmt::Debug for Link<C, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl<C, A> Link<C, A>
where
    C: Compound<A>,
    C::Leaf: Canon,
    A: Canon,
{
    /// Create a new link
    pub fn new(compound: C) -> Self
    where
        A: Annotation<C::Leaf>,
    {
        Link {
            inner: RwLock::new(LinkInner::C(Arc::new(compound))),
        }
    }

    /// Creates a new link from an id and annotation
    pub fn new_persisted(id: Id, annotation: A) -> Self {
        Link {
            inner: RwLock::new(LinkInner::Ia(id, annotation)),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> LinkAnnotation<C, A>
    where
        A: Annotation<C::Leaf>,
    {
        let borrow = self.inner.upgradeable_read();
        let a = match *borrow {
            LinkInner::Ca(_, _)
            | LinkInner::Ica(_, _, _)
            | LinkInner::Ia(_, _) => return LinkAnnotation(borrow.downgrade()),
            LinkInner::C(ref c) | LinkInner::Ic(_, ref c) => {
                A::combine(c.annotations())
            }
            LinkInner::Placeholder => unreachable!(),
        };

        let mut borrow = borrow.upgrade();

        match mem::replace(&mut *borrow, LinkInner::Placeholder) {
            LinkInner::C(c) => *borrow = LinkInner::Ca(c, a),
            LinkInner::Ic(i, c) => *borrow = LinkInner::Ica(i, c, a),
            _ => unreachable!(),
        }

        let borrow = borrow.downgrade();
        LinkAnnotation(borrow)
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn into_compound(self) -> Result<C, CanonError>
    where
        C::Leaf: Canon,
    {
        // assure inner value is loaded
        let _ = self.inner()?;

        let inner = self.inner.into_inner();
        match inner {
            LinkInner::C(rc)
            | LinkInner::Ca(rc, _)
            | LinkInner::Ic(_, rc)
            | LinkInner::Ica(_, rc, _) => match Arc::try_unwrap(rc) {
                Ok(c) => Ok(c),
                Err(rc) => Ok((&*rc).clone()),
            },
            _ => unreachable!(),
        }
    }

    /// Computes the Id of the link
    pub fn id(&self) -> Id
    where
        C::Leaf: Canon,
        A: Annotation<C::Leaf>,
    {
        let borrow = self.inner.upgradeable_read();

        match &*borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(c) | LinkInner::Ca(c, _) => {
                #[cfg(not(feature = "persistence"))]
                let id = Id::new(&c.generic());

                #[cfg(feature = "persistence")]
                let id = Persistence::persist_default(&**c)
                    .expect("TODO, handle error")
                    .into_inner();

                let mut borrow = borrow.upgrade();

                match mem::replace(&mut *borrow, LinkInner::Placeholder) {
                    LinkInner::C(c) => *borrow = LinkInner::Ic(id, c),
                    LinkInner::Ca(c, a) => *borrow = LinkInner::Ica(id, c, a),
                    _ => unreachable!(),
                };
                id
            }
            LinkInner::Ia(id, _)
            | LinkInner::Ic(id, _)
            | LinkInner::Ica(id, _, _) => *id,
        }
    }

    /// See doc for `inner`
    #[deprecated(since = "0.10.0", note = "Please use `inner` instead")]
    pub fn compound(&self) -> Result<LinkCompound<C, A>, CanonError> {
        self.inner()
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner(&self) -> Result<LinkCompound<C, A>, CanonError> {
        let borrow = self.inner.upgradeable_read();

        match *borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(_)
            | LinkInner::Ca(_, _)
            | LinkInner::Ic(_, _)
            | LinkInner::Ica(_, _, _) => {
                return Ok(LinkCompound(borrow.downgrade()))
            }
            LinkInner::Ia(id, _) => {
                // First we check if the value is available to be reified
                // directly
                match id.reify::<GenericTree>() {
                    Ok(generic) => {
                        // re-borrow mutable
                        let mut borrow = borrow.upgrade();
                        if let LinkInner::Ia(id, anno) =
                            mem::replace(&mut *borrow, LinkInner::Placeholder)
                        {
                            let value = C::from_generic(&generic)?;
                            *borrow = LinkInner::Ica(id, Arc::new(value), anno);

                            // re-borrow immutable
                            let borrow = borrow.downgrade();

                            Ok(LinkCompound(borrow))
                        } else {
                            unreachable!(
                                "Guaranteed to match the same as above"
                            )
                        }
                    }
                    Err(CanonError::NotFound) => {
                        // Value was not able to be reified, if we're using
                        // persistance we look in the backend, otherwise we
                        // return an `Err(NotFound)`

                        #[cfg(feature = "persistence")]
                        {
                            // re-borrow mutable
                            let mut borrow = borrow.upgrade();
                            if let LinkInner::Ia(id, anno) = mem::replace(
                                &mut *borrow,
                                LinkInner::Placeholder,
                            ) {
                                match Persistence::get(&id) {
                                    Ok(generic) => {
                                        let compound =
                                            C::from_generic(&generic)?;
                                        *borrow = LinkInner::Ica(
                                            id,
                                            Arc::new(compound),
                                            anno,
                                        );

                                        // re-borrow immutable
                                        let borrow = borrow.downgrade();

                                        Ok(LinkCompound(borrow))
                                    }
                                    Err(PersistError::Canon(e)) => Err(e),
                                    _err @ Err(_) => {
                                        // TODO: log errors to the backend
                                        Err(CanonError::NotFound)
                                    }
                                }
                            } else {
                                unreachable!(
                                    "Guaranteed to match the same as above"
                                )
                            }
                        }
                        #[cfg(not(feature = "persistence"))]
                        Err(CanonError::NotFound)
                    }
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// See doc for `inner_mut`
    #[deprecated(since = "0.10.0", note = "Please use `inner` instead")]
    pub fn compound_mut(&mut self) -> Result<LinkCompoundMut<C, A>, CanonError>
    where
        C: Canon,
    {
        self.inner_mut()
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner_mut(&mut self) -> Result<LinkCompoundMut<C, A>, CanonError>
    where
        C: Canon,
    {
        // assure inner value is loaded
        let _ = self.inner()?;

        let mut borrow: RwLockWriteGuard<LinkInner<C, A>> = self.inner.write();

        match mem::replace(&mut *borrow, LinkInner::Placeholder) {
            LinkInner::C(c)
            | LinkInner::Ca(c, _)
            | LinkInner::Ic(_, c)
            | LinkInner::Ica(_, c, _) => {
                // clear all cached data
                *borrow = LinkInner::C(c);
            }
            _ => unreachable!(),
        }
        Ok(LinkCompoundMut(borrow))
    }
}

impl<C, A> Canon for Link<C, A>
where
    C: Compound<A> + Canon,
    C::Leaf: Canon,
    A: Annotation<C::Leaf>,
{
    fn encode(&self, sink: &mut Sink) {
        self.id().encode(sink);
        self.annotation().encode(sink);
    }

    fn decode(source: &mut Source) -> Result<Self, CanonError> {
        let id = Id::decode(source)?;
        let a = A::decode(source)?;
        Ok(Link {
            inner: RwLock::new(LinkInner::Ia(id, a)),
        })
    }

    fn encoded_len(&self) -> usize {
        self.id().encoded_len() + self.annotation().encoded_len()
    }
}

/// A wrapped borrow of an inner link guaranteed to contain a computed
/// annotation
#[derive(Debug)]
pub struct LinkAnnotation<'a, C, A>(RwLockReadGuard<'a, LinkInner<C, A>>);

/// A wrapped borrow of an inner node guaranteed to contain a compound node
#[derive(Debug)]
pub struct LinkCompound<'a, C, A>(RwLockReadGuard<'a, LinkInner<C, A>>);

/// A wrapped mutable borrow of an inner node guaranteed to contain a compound
/// node
#[derive(Debug)]
pub struct LinkCompoundMut<'a, C, A>(RwLockWriteGuard<'a, LinkInner<C, A>>);

impl<'a, C, A> Deref for LinkAnnotation<'a, C, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        match *self.0 {
            LinkInner::Ica(_, _, ref a)
            | LinkInner::Ia(_, ref a)
            | LinkInner::Ca(_, ref a) => a,
            _ => unreachable!(),
        }
    }
}

impl<'a, C, A> Deref for LinkCompound<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match *self.0 {
            LinkInner::C(ref c)
            | LinkInner::Ca(ref c, _)
            | LinkInner::Ic(_, ref c)
            | LinkInner::Ica(_, ref c, _) => c,
            _ => unreachable!(),
        }
    }
}

impl<'a, C, A> Deref for LinkCompoundMut<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match *self.0 {
            LinkInner::C(ref c) => c,
            _ => unreachable!(),
        }
    }
}

impl<'a, C, A> DerefMut for LinkCompoundMut<'a, C, A>
where
    C: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match *self.0 {
            LinkInner::C(ref mut c) => Arc::make_mut(c),
            _ => unreachable!(),
        }
    }
}
