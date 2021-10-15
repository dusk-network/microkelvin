// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::rc::Rc;
use core::cell::RefCell;
use rkyv::ser::Serializer;

use owning_ref::OwningRef;
use rkyv::{Archive, Deserialize, Serialize};
use rkyv::{Fallible, Infallible};

use crate::primitive::Primitive;

use crate::chonker::{Offset, RawOffset};
use crate::{ARef, Annotation, ArchivedCompound, Chonker, Compound};

pub enum NodeRef<'a, C>
where
    C: Archive,
{
    Memory(&'a C),
    Archived(&'a C::Archived),
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
        chonker: Chonker,
    },
}

pub struct ArchivedLink<A>(RawOffset, A);

impl<C, A> Archive for Link<C, A> {
    type Archived = ArchivedLink<A>;
    type Resolver = ArchivedLink<A>;

    unsafe fn resolve(
        &self,
        _pos: usize,
        resolver: Self::Resolver,
        out: *mut <Self as Archive>::Archived,
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
        Link::Memory {
            rc: Rc::new(C::default()),
            annotation: RefCell::new(None),
        }
    }
}

impl<C, A> Link<C, A>
where
    C: Compound<A>,
    C::Archived: ArchivedCompound<C, A>,
    A: Primitive + Annotation<C::Leaf>,
{
    /// Create a new link
    pub fn new(compound: C) -> Self {
        Link::Memory {
            rc: Rc::new(compound),
            annotation: RefCell::new(None),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> ARef<A> {
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
    pub fn inner(&self) -> NodeRef<C> {
        match self {
            Link::Memory { rc, .. } => NodeRef::Memory(&(*rc)),
            Link::Archived { ofs, chonker, .. } => {
                NodeRef::Archived(chonker.get(*ofs))
            }
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    pub fn inner_mut(&mut self) -> &mut C
    where
        C: Clone,
        C::Archived: Deserialize<C, Infallible>,
    {
        match self {
            Link::Memory { rc, annotation } => {
                *annotation.borrow_mut() = None;
                return Rc::make_mut(rc);
            }
            Link::Archived { ofs, chonker, .. } => {
                let archived = chonker.get(*ofs);
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
