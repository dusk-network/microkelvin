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

use crate::storage::{Offset, RawOffset, Storage};
use crate::{ARef, Annotation, ArchivedCompound, Compound, Portal};

pub enum NodeRef<'a, C, CA> {
    Memory(&'a C),
    Archived(&'a CA),
}

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
        ofs: Offset<C>,
        /// the final annotation
        a: A,
        /// link to the chonky boi
        portal: Portal,
    },
}

pub struct ArchivedLink<A>(RawOffset, A);

impl<C, A> Archive for Link<C, A> {
    type Archived = ArchivedLink<A>;
    type Resolver = (RawOffset, A);

    unsafe fn resolve(
        &self,
        _pos: usize,
        resolver: Self::Resolver,
        out: *mut <Self as Archive>::Archived,
    ) {
        *out = ArchivedLink(resolver.0, resolver.1)
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
                let ofs = serializer.borrow_mut().put(to_insert).into_raw();
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
    pub fn inner(&self) -> NodeRef<C, C::Archived>
    where
        C: Archive,
    {
        match self {
            Link::Memory { rc, .. } => NodeRef::Memory(&(*rc)),
            Link::Archived { ofs, portal, .. } => {
                NodeRef::Archived(portal.get(*ofs))
            }
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    pub fn inner_mut(&mut self) -> &mut C
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
            Link::Archived { ofs, portal, .. } => {
                let archived = portal.get(*ofs);
                let c = archived.deserialize(&mut rkyv::Infallible).unwrap();
                *self = Link::Memory {
                    rc: Rc::new(c),
                    annotation: RefCell::new(None),
                };
                self.inner_mut()
            }
        }
    }
}
