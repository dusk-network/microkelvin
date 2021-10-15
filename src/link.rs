// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::{Ref, RefCell, RefMut};
use core::mem;
use rkyv::ser::Serializer;

use owning_ref::{OwningRef, OwningRefMut};
use rkyv::Fallible;
use rkyv::{Archive, Deserialize, Serialize};

use crate::primitive::Primitive;

use crate::chonker::{Offset, RawOffset};
use crate::{Annotation, ArchivedChildren, Child, Chonker, Compound};

pub type NodeAnnotation<'a, C, A> = OwningRef<Ref<'a, LinkInner<C, A>>, A>;

pub enum NodeRef<'a, C, A>
where
    C: Archive,
{
    Archived(OwningRef<Ref<'a, LinkInner<C, A>>, (C::Archived, A)>),
    Referenced(OwningRef<Ref<'a, LinkInner<C, A>>, (C, A)>),
}

impl<'a, C, A> std::fmt::Debug for NodeRef<'a, C, A>
where
    C: Archive,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Archived(_) => write!(f, "Archived"),
            Self::Referenced(_) => write!(f, "Referenced"),
        }
    }
}

impl<'a, C, A> NodeRef<'a, C, A>
where
    C: Compound<A>,
    C::Archived: ArchivedChildren<C, A>,
    A: Archive<Archived = A> + Annotation<C::Leaf>,
{
    pub fn child(&self, ofs: usize) -> Child<'a, C, A> {
        match self {
            NodeRef::Archived(a) => (*a).0.child(ofs),
            NodeRef::Referenced(r) => (*r).0.child(ofs),
        }
    }

    pub fn annotation(&self) -> NodeAnnotation<'a, C, A> {
        match self {
            NodeRef::Archived(a) => a,
            NodeRef::Referenced(r) => r,
        }
    }
}

type NodeRefMut<'a, C, A> = OwningRefMut<RefMut<'a, LinkInner<C, A>>, C>;

#[derive(Clone, Debug)]
pub enum LinkInner<C, A> {
    Placeholder,
    C(Rc<C>),
    Ca(Rc<C>, A),
    Io(Offset<C>, A, Chonker),
}

impl<C, A> Default for LinkInner<C, A> {
    fn default() -> Self {
        LinkInner::Placeholder
    }
}

#[derive(Clone, Debug)]
/// The Link struct is an annotated merkle link to a compound type
///
/// The link takes care of lazily evaluating the annotation of the inner type,
/// and to load it from memory or backend when needed.
pub struct Link<C, A> {
    inner: RefCell<LinkInner<C, A>>,
}

pub struct ArchivedLink<A>(RawOffset, A);

impl<C, A> Archive for Link<C, A> {
    type Archived = ArchivedLink<A>;
    type Resolver = ArchivedLink<A>;

    unsafe fn resolve(
        &self,
        _pos: usize,
        resolver: Self::Resolver,
        out: *mut Self::Archived,
    ) {
        *out = resolver
    }
}

impl<C, A, S> Deserialize<Link<C, A>, S> for ArchivedLink<A>
where
    A: Archive + Clone,
    A::Archived: Deserialize<A, S>,
    S: Fallible,
{
    fn deserialize(
        &self,
        _de: &mut S,
    ) -> Result<Link<C, A>, <S as Fallible>::Error> {
        todo!()
    }
}

impl<C, A, S> Serialize<S> for Link<C, A>
where
    C: Compound<A> + Serialize<S>,
    A: Primitive + Annotation<C::Leaf> + Serialize<S>,
    S: Serializer + Fallible,
{
    fn serialize(
        &self,
        _serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        todo!()
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

impl<C, A> Link<C, A>
where
    C: Compound<A>,
    A: Primitive + Annotation<C::Leaf>,
{
    /// Create a new link
    pub fn new(compound: C) -> Self {
        Link {
            inner: RefCell::new(LinkInner::C(Rc::new(compound))),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> NodeAnnotation<C, A> {
        let mut borrow = self.inner.borrow_mut();
        *borrow = match mem::replace(&mut *borrow, LinkInner::Placeholder) {
            LinkInner::C(c) => {
                let a = A::combine(c.annotations());
                LinkInner::Ca(c, a)
            }
            other @ _ => other,
        };

        drop(borrow);

        OwningRef::new(self.inner.borrow()).map(|brw| match brw {
            LinkInner::Ca(_, a) | LinkInner::Io(_, a, _) => a,
            _ => unreachable!(),
        })
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
            LinkInner::Io(_, _, _) => {
                todo!()
            }
            _ => unreachable!(),
        }
    }

    /// Returns a reference to the inner node, possibly in its archived form
    pub fn inner(&self) -> NodeRef<C, A> {
        let borrow: Ref<LinkInner<C, A>> = self.inner.borrow();

        match &*borrow {
            LinkInner::C(c) | LinkInner::Ca(c, _) => NodeRef::Referenced(
                OwningRef::new(borrow).map(|brw| match brw {
                    LinkInner::C(c) | LinkInner::Ca(c, _) => &(**c),
                    _ => unreachable!("fixme: make unchecked?"),
                }),
            ),
            LinkInner::Io(_, _, _) => {
                NodeRef::Archived(OwningRef::new(borrow).map(|brw| match brw {
                    LinkInner::Io(ofs, _, ch) => ch.get(*ofs),
                    _ => unreachable!("fixme: make unchecked?"),
                }))
            }
            LinkInner::Placeholder => unreachable!(),
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    pub fn inner_mut(&mut self) -> NodeRefMut<C, A>
    where
        C: Clone,
    {
        let mut borrow: RefMut<LinkInner<C, A>> = self.inner.borrow_mut();

        match mem::replace(&mut *borrow, LinkInner::Placeholder) {
            LinkInner::C(c) | LinkInner::Ca(c, _) => {
                // clear all cached data
                *borrow = LinkInner::C(c);

                OwningRefMut::new(borrow).map_mut(|b| {
                    if let LinkInner::C(c) = b {
                        Rc::make_mut(c)
                    } else {
                        unreachable!()
                    }
                })
            }
            _ => unreachable!(),
        }
    }
}
