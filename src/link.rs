// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::mem;
use core::ops::{Deref, DerefMut};
use rkyv::validation::validators::DefaultValidator;
use rkyv::Fallible;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use crate::backend::{
    Getable, PortalDeserializer, PortalProvider, PortalSerializer,
};
use crate::error::Error;
use crate::id::{Id, IdHash};

use crate::{Annotation, Compound};

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum LinkInner<C, A> {
    Placeholder,
    C(Rc<C>),
    Ca(Rc<C>, A),
    Ia(Id<C>, A),
    #[allow(unused)]
    Ica(Id<C>, Rc<C>, A),
}

#[derive(Clone)]
/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub struct Link<C, A> {
    inner: RefCell<LinkInner<C, A>>,
}

#[derive(CheckBytes)]
pub struct ArchivedLink<A: Archive>(<IdHash as Archive>::Archived, A::Archived);

impl<C, A> Archive for Link<C, A>
where
    A: Archive,
{
    type Archived = ArchivedLink<A>;
    type Resolver = (Id<C>, A);

    unsafe fn resolve(
        &self,
        _pos: usize,
        _resolver: Self::Resolver,
        _out: *mut Self::Archived,
    ) {
        // *out = todo!();
        todo!()
    }
}

impl<C, A> Deserialize<Link<C, A>, PortalDeserializer> for ArchivedLink<A>
where
    A: Archive + Clone,
    A::Archived: Deserialize<A, PortalDeserializer>,
{
    fn deserialize(
        &self,
        de: &mut PortalDeserializer,
    ) -> Result<Link<C, A>, Error> {
        let id = Id::new_from_hash(self.0, de.portal());
        let anno = self.1.deserialize(de)?;
        Ok(Link {
            inner: RefCell::new(LinkInner::Ia(id, anno)),
        })
    }
}

impl<C, A> Serialize<PortalSerializer> for Link<C, A>
where
    C: Compound<A> + Archive,
    C::Archived: for<'a> CheckBytes<DefaultValidator<'a>>
        + Deserialize<C, PortalDeserializer>,
    A: Clone + Archive + Annotation<C::Leaf> + Serialize<PortalSerializer>,
    A::Archived: for<'a> CheckBytes<DefaultValidator<'a>>
        + Deserialize<A, PortalDeserializer>,
{
    fn serialize(
        &self,
        provider: &mut PortalSerializer,
    ) -> Result<Self::Resolver, <PortalSerializer as Fallible>::Error> {
        let anno = self.annotation().clone();
        let portal = provider.portal();
        let to_put = &*self.inner()?;
        let id = to_put.put(portal)?;
        Ok((id, anno))
    }
}

impl<C, A> Default for Link<C, A>
where
    C: Default,
{
    fn default() -> Self {
        Link {
            inner: RefCell::new(LinkInner::C(Rc::new(C::default()))),
        }
    }
}

impl<C: core::fmt::Debug, A: core::fmt::Debug> core::fmt::Debug for Link<C, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl<C, A> Link<C, A> {
    /// Create a new link
    pub fn new(compound: C) -> Self {
        Link {
            inner: RefCell::new(LinkInner::C(Rc::new(compound))),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> LinkAnnotation<C, A>
    where
        C: Compound<A>,
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

    /// Consumes the link and returns the inner Compound value
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn unlink(self) -> Result<C, Error>
    where
        C: Getable + Clone,
    {
        // assure inner value is loaded
        let _ = self.inner()?;

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

    /// See doc for `inner`
    #[deprecated(since = "0.10.0", note = "Please use `inner` instead")]
    pub fn compound(&self) -> Result<LinkCompound<C, A>, Error>
    where
        C: Getable,
    {
        self.inner()
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner(&self) -> Result<LinkCompound<C, A>, Error>
    where
        C: Getable,
    {
        let borrow = self.inner.borrow();

        match *borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(_) | LinkInner::Ca(_, _) | LinkInner::Ica(_, _, _) => {
                return Ok(LinkCompound(borrow))
            }
            LinkInner::Ia(ref id, _) => {
                let inner = id.reify()?;
                // re-borrow mutable
                drop(borrow);
                let mut borrow = self.inner.borrow_mut();
                if let LinkInner::Ia(id, anno) =
                    mem::replace(&mut *borrow, LinkInner::Placeholder)
                {
                    *borrow = LinkInner::Ica(id, Rc::new(inner), anno);

                    // re-borrow immutable
                    drop(borrow);
                    let borrow = self.inner.borrow();

                    Ok(LinkCompound(borrow))
                } else {
                    unreachable!("Guaranteed to match the same as above")
                }
            }
        }
    }

    /// See doc for `inner_mut`
    #[deprecated(since = "0.10.0", note = "Please use `inner` instead")]
    pub fn compound_mut(&mut self) -> Result<LinkCompoundMut<C, A>, Error>
    where
        C: Getable,
    {
        self.inner_mut()
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner_mut(&mut self) -> Result<LinkCompoundMut<C, A>, Error>
    where
        C: Getable,
    {
        // assure inner value is loaded
        let _ = self.inner()?;
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
