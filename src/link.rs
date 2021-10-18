// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::RefCell;
use rkyv::ser::Serializer;
use std::borrow::BorrowMut;

use owning_ref::OwningRef;
use rkyv::{Archive, Deserialize, Serialize};
use rkyv::{Fallible, Infallible};

use crate::storage::{Storage, Stored};
use crate::{ARef, AWrap, Annotation, ArchivedCompound, Compound, Portal};

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
    /// A Link to an archived node
    Archived {
        /// archived at offset
        stored: Stored<C>,
        /// the final annotation
        a: A,
    },
}

pub struct ArchivedLink<C, A>(Stored<C>, A);

impl<C, A> ArchivedLink<C, A> {
    pub fn annotation(&self) -> &A {
        &self.1
    }

    pub fn inner<'a>(&self, portal: &'a Portal) -> &'a C::Archived
    where
        C: Archive,
    {
        portal.get::<C>(self.0)
    }
}

impl<C, A> Archive for Link<C, A> {
    type Archived = ArchivedLink<C, A>;
    type Resolver = (Stored<C>, A);

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
    C: Archive,
    A: Archive + Clone,
    A::Archived: Deserialize<A, D>,
    D: Fallible,
{
    fn deserialize(
        &self,
        _: &mut D,
    ) -> Result<Link<C, A>, <D as Fallible>::Error> {
        Ok(Link::Archived {
            stored: self.0,
            a: self.1.clone(),
        })
    }
}

impl<C, A, S> Serialize<S> for Link<C, A>
where
    C: Compound<A> + Serialize<S> + Serialize<Storage>,
    A: Annotation<C::Leaf>,
    S: Serializer + BorrowMut<Storage>,
{
    fn serialize(
        &self,
        serializer: &mut S,
    ) -> Result<Self::Resolver, S::Error> {
        match self {
            Link::Memory { rc, annotation } => {
                let a = if let Some(a) = &*annotation.borrow() {
                    a.clone()
                } else {
                    todo!()
                };
                let to_insert = &(**rc);
                let ofs = serializer.borrow_mut().put(to_insert);
                Ok((ofs, a))
            }
            Link::Archived { .. } => todo!(),
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
        C: Archive + Compound<A>,
        C::Archived: ArchivedCompound<C, A>,
        A: Annotation<C::Leaf>,
    {
        match self {
            Link::Memory { annotation, rc } => {
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
                    *annotation.borrow_mut() =
                        Some(A::combine(rc.annotations()));
                    self.annotation()
                }
            }
            Link::Archived { a, .. } => ARef::Borrowed(a),
        }
    }

    /// Consumes the link and returns the inner Compound value
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn unlink(self) -> C
    where
        C: Clone,
    {
        match self {
            Link::Memory { rc, .. } => match Rc::try_unwrap(rc) {
                Ok(c) => c,
                Err(rc) => (&*rc).clone(),
            },
            Link::Archived { .. } => todo!(),
        }
    }

    /// Returns a reference to the inner node, possibly in its archived form
    pub fn inner<'a>(&'a self, portal: &'a Portal) -> AWrap<'a, C>
    where
        C: Archive,
    {
        match self {
            Link::Memory { rc, .. } => AWrap::Memory(&(*rc)),
            Link::Archived { stored, .. } => {
                AWrap::Archived(portal.get(*stored))
            }
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    pub fn inner_mut(&mut self, portal: &Portal) -> &mut C
    where
        C: Archive + Clone,
        C::Archived: Deserialize<C, Infallible>,
    {
        match self {
            Link::Memory { rc, annotation } => {
                // clear annotation
                annotation.borrow_mut().take();
                return Rc::make_mut(rc);
            }
            Link::Archived { stored, .. } => {
                let ca = portal.get(*stored);
                let c = ca.deserialize(&mut Infallible).expect("Infallible");
                *self = Link::Memory {
                    rc: Rc::new(c),
                    annotation: RefCell::new(None),
                };
                self.inner_mut(portal)
            }
        }
    }
}
