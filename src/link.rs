// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::mem;
use core::ops::{Deref, DerefMut};

use canonical::{Canon, CanonError, Id, Sink, Source};

#[cfg(feature = "persistence")]
use crate::persist::{PersistError, Persistence};

use crate::generic::GenericTree;
use crate::{Annotation, Compound};

#[derive(Debug, Clone)]
enum LinkInner<C, A> {
    Placeholder,
    C(Rc<C>),
    Ca(Rc<C>, A),
    Ia(Id, A),
    #[allow(unused)]
    Ica(Id, Rc<C>, A),
}

#[derive(Clone)]
/// TODO
pub struct Link<C, A> {
    inner: RefCell<LinkInner<C, A>>,
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
            inner: RefCell::new(LinkInner::C(Rc::new(compound))),
        }
    }

    /// Creates a new link from an id and annotation
    pub fn new_persisted(id: Id, annotation: A) -> Self {
        Link {
            inner: RefCell::new(LinkInner::Ia(id, annotation)),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> LinkAnnotation<C, A>
    where
        A: Annotation<C::Leaf>,
    {
        let borrow = self.inner.borrow();
        let a = match *borrow {
            LinkInner::Ca(_, _)
            | LinkInner::Ica(_, _, _)
            | LinkInner::Ia(_, _) => return LinkAnnotation(borrow),
            LinkInner::C(ref c) => A::combine(c.annotations()),
            LinkInner::Placeholder => unreachable!(),
        };

        drop(borrow);
        let mut borrow = self.inner.borrow_mut();

        if let LinkInner::C(c) =
            mem::replace(&mut *borrow, LinkInner::Placeholder)
        {
            *borrow = LinkInner::Ca(c, a)
        } else {
            unreachable!()
        }
        drop(borrow);
        let borrow = self.inner.borrow();
        LinkAnnotation(borrow)
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn into_compound(self) -> Result<C, CanonError>
    where
        C::Leaf: Canon,
    {
        // assure compound is loaded
        let _ = self.compound()?;

        let inner = self.inner.into_inner();
        match inner {
            LinkInner::C(rc)
            | LinkInner::Ca(rc, _)
            | LinkInner::Ica(_, rc, _) => match Rc::try_unwrap(rc) {
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
        let borrow = self.inner.borrow();
        match &*borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(c) | LinkInner::Ca(c, _) => {
                let gen = c.generic();
                Id::new(&gen)
            }
            LinkInner::Ia(id, _) | LinkInner::Ica(id, _, _) => *id,
        }
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn compound(&self) -> Result<LinkCompound<C, A>, CanonError> {
        let borrow = self.inner.borrow();

        match *borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(_) | LinkInner::Ca(_, _) | LinkInner::Ica(_, _, _) => {
                return Ok(LinkCompound(borrow))
            }
            LinkInner::Ia(id, _) => {
                // First we check if the value is available to be reified
                // directly
                match id.reify::<GenericTree>() {
                    Ok(generic) => {
                        // re-borrow mutable
                        drop(borrow);
                        let mut borrow = self.inner.borrow_mut();
                        if let LinkInner::Ia(id, anno) =
                            mem::replace(&mut *borrow, LinkInner::Placeholder)
                        {
                            let value = C::from_generic(&generic)?;
                            *borrow = LinkInner::Ica(id, Rc::new(value), anno);

                            // re-borrow immutable
                            drop(borrow);
                            let borrow = self.inner.borrow();

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
                            drop(borrow);
                            let mut borrow = self.inner.borrow_mut();
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
                                            Rc::new(compound),
                                            anno,
                                        );

                                        // re-borrow immutable
                                        drop(borrow);
                                        let borrow = self.inner.borrow();

                                        Ok(LinkCompound(borrow))
                                    }
                                    Err(PersistError::Canon(e)) => Err(e),
                                    err
                                    @
                                    Err(
                                        PersistError::Io(_)
                                        | PersistError::Other(_),
                                    ) => {
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

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn compound_mut(&mut self) -> Result<LinkCompoundMut<C, A>, CanonError>
    where
        C: Canon,
    {
        // assure compound is loaded
        let _ = self.compound()?;

        let mut borrow: RefMut<LinkInner<C, A>> = self.inner.borrow_mut();

        match mem::replace(&mut *borrow, LinkInner::Placeholder) {
            LinkInner::C(c) | LinkInner::Ca(c, _) | LinkInner::Ica(_, c, _) => {
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
            inner: RefCell::new(LinkInner::Ia(id, a)),
        })
    }

    fn encoded_len(&self) -> usize {
        self.id().encoded_len() + self.annotation().encoded_len()
    }
}

/// A wrapped borrow of an inner link guaranteed to contain a computed
/// annotation
#[derive(Debug)]
pub struct LinkAnnotation<'a, C, A>(Ref<'a, LinkInner<C, A>>);

/// A wrapped borrow of an inner node guaranteed to contain a compound node
#[derive(Debug)]
pub struct LinkCompound<'a, C, A>(Ref<'a, LinkInner<C, A>>);

/// A wrapped mutable borrow of an inner node guaranteed to contain a compound
/// node
#[derive(Debug)]
pub struct LinkCompoundMut<'a, C, A>(RefMut<'a, LinkInner<C, A>>);

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
            LinkInner::C(ref mut c) => Rc::make_mut(c),
            _ => unreachable!(),
        }
    }
}
