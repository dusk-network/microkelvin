// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::mem;
use core::ops::{Deref, DerefMut};

#[cfg(feature = "persistence")]
use crate::persist::{PersistError, Persistence};

use crate::id::Id;
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
/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub struct Link<C, A> {
    inner: RefCell<LinkInner<C, A>>,
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

impl<C, A> Link<C, A>
where
    C: Compound<A>,
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
    pub fn into_compound(self) -> Result<C, ()> {
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

    /// Computes the Id of the link
    pub fn id(&self) -> Id
    where
        A: Annotation<C::Leaf>,
    {
        let borrow = self.inner.borrow();
        match &*borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(c) | LinkInner::Ca(c, _) => {
                // let gen = c.generic();
                // Id::new(&gen)
                todo!()
            }
            LinkInner::Ia(id, _) | LinkInner::Ica(id, _, _) => *id,
        }
    }

    /// See doc for `inner`
    #[deprecated(since = "0.10.0", note = "Please use `inner` instead")]
    pub fn compound(&self) -> Result<LinkCompound<C, A>, ()> {
        self.inner()
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner(&self) -> Result<LinkCompound<C, A>, ()> {
        let borrow = self.inner.borrow();

        match *borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(_) | LinkInner::Ca(_, _) | LinkInner::Ica(_, _, _) => {
                return Ok(LinkCompound(borrow))
            }
            LinkInner::Ia(id, _) => {
                // First we check if the value is available to be reified
                // directly
                match id.reify::<()>() {
                    Ok(generic) => {
                        // re-borrow mutable
                        drop(borrow);
                        let mut borrow = self.inner.borrow_mut();
                        if let LinkInner::Ia(id, anno) =
                            mem::replace(&mut *borrow, LinkInner::Placeholder)
                        {
                            let value = todo!();
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
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// See doc for `inner_mut`
    #[deprecated(since = "0.10.0", note = "Please use `inner` instead")]
    pub fn compound_mut(&mut self) -> Result<LinkCompoundMut<C, A>, ()> {
        self.inner_mut()
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn inner_mut(&mut self) -> Result<LinkCompoundMut<C, A>, ()> {
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
