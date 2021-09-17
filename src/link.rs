// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::mem;
use core::ops::{Deref, DerefMut};
use rkyv::ser::Serializer;
use rkyv::{out_field, AlignedVec, Fallible};

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use crate::backend::PortalProvider;
use crate::id::{Id, IdHash};

use crate::{Annotation, Compound, Portal};

#[derive(Debug, Clone)]
#[repr(u8)]
pub enum LinkInner<C, A> {
    Placeholder,
    C(Rc<C>),
    Ca(Rc<C>, A),
    Ia(Id<C>, A),
}

#[derive(Clone)]
/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub struct Link<C, A> {
    inner: RefCell<LinkInner<C, A>>,
}

#[derive(CheckBytes, Debug)]
pub struct ArchivedLink<A: Archive>(IdHash, A::Archived);

impl<C, A> Archive for Link<C, A>
where
    C: Compound<A>,
    A: Archive + Annotation<C::Leaf>,
{
    type Archived = ArchivedLink<A>;
    type Resolver = (IdHash, A::Resolver);

    unsafe fn resolve(
        &self,
        pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        (*out).0 = resolver.0;
        let (fp, fo) = out_field!(out.1);
        let a = &*self.annotation();
        a.resolve(pos + fp, resolver.1, fo);
    }
}

impl<C, A, S> Deserialize<Link<C, A>, S> for ArchivedLink<A>
where
    A: Archive + Clone,
    A::Archived: Deserialize<A, S>,
    S: Fallible + PortalProvider,
{
    fn deserialize(
        &self,
        de: &mut S,
    ) -> Result<Link<C, A>, <S as Fallible>::Error> {
        let id = Id::new_from_hash(self.0, de.portal());
        let anno = self.1.deserialize(de)?;
        Ok(Link {
            inner: RefCell::new(LinkInner::Ia(id, anno)),
        })
    }
}

impl<C, A, S> Serialize<S> for Link<C, A>
where
    C: Compound<A> + Serialize<S>,
    A: Clone + Archive + Annotation<C::Leaf> + Serialize<S>,
    S: Serializer + Fallible + PortalProvider + From<Portal> + Into<AlignedVec>,
    S::Error: core::fmt::Debug,
{
    fn serialize(
        &self,
        serializer: &mut S,
    ) -> Result<Self::Resolver, <S as Fallible>::Error> {
        let anno = &*self.annotation();
        let a_resolver = match anno.serialize(serializer) {
            Ok(r) => r,
            _ => unreachable!(),
        };
        let portal = serializer.portal();
        let id = self.id();
        Ok((id.hash().clone(), a_resolver))
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

enum LinkRef<'a, C, A>
where
    C: Archive,
{
    InMemory(LinkCompound<'a, C, A>),
    Archived(&'a C::Archived),
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
            LinkInner::Ca(_, _) | LinkInner::Ia(_, _) => {
                return LinkAnnotation(borrow)
            }
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
    pub fn unlink(self) -> C
    where
        C: Clone,
    {
        let inner = self.inner.into_inner();
        match inner {
            LinkInner::C(rc) | LinkInner::Ca(rc, _) => match Rc::try_unwrap(rc)
            {
                Ok(c) => c,
                Err(rc) => (&*rc).clone(),
            },
            LinkInner::Ia(id, _) => {
                todo!()
            }
            _ => unreachable!(),
        }
    }

    pub fn id(&self) -> Id<C> {
        todo!()
    }

    /// Gets a reference to the inner compound of the link'
    pub fn inner(&self) -> LinkRef<C, A>
    where
        C: Archive,
    {
        let borrow = self.inner.borrow();

        match *borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(c) | LinkInner::Ca(c, _) => LinkRef::rc(c.clone()),
            LinkInner::Ia(ref id, _) => LinkRef::archived(id.resolve()),
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner_mut(&mut self) -> LinkCompoundMut<C, A> {
        // assure inner value is loaded
        let _ = self.inner();
        let mut borrow: RefMut<LinkInner<C, A>> = self.inner.borrow_mut();

        match mem::replace(&mut *borrow, LinkInner::Placeholder) {
            LinkInner::C(c) | LinkInner::Ca(c, _) => {
                // clear all cached data
                *borrow = LinkInner::C(c);
            }
            _ => unreachable!(),
        }
        LinkCompoundMut(borrow)
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
