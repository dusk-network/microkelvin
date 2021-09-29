// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Microkelvin
//!
//! A library for dealing with tree-shaped data. It has three parts:
//!
//! `Compound`, a trait for a generic way to implement tree structures
//! `Annotation`, a trait for annotated subtrees used for searching
//! `Branch` and `BranchMut`, types for representing branches in tree-formed
//! data as well as methods of search.

// #![cfg_attr(not(feature = "host"), no_std)]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

#[macro_use]
extern crate alloc;

mod annotations;
mod backend;
mod branch;
mod branch_mut;
mod compound;
mod id;
mod link;
mod primitive;
mod walk;

pub use annotations::{
    Annotation, Cardinality, Combine, GetMaxKey, Keyed, MaxKey, Nth,
};
pub use backend::{
    Backend, Portal, PortalDeserializer, PortalProvider, PortalSerializer,
};
pub use branch::Branch;
pub use branch_mut::BranchMut;
pub use compound::{
    AnnoIter, ArchivedChild, ArchivedChildren, Child, ChildMut, Compound,
    MutableLeaves,
};
pub use id::Id;
pub use link::{Link, LinkAnnotation, LinkCompound, LinkCompoundMut};
pub use primitive::Primitive;
pub use walk::{First, Step, Walker};

#[cfg(feature = "host")]
mod disk;
#[cfg(feature = "host")]
pub use disk::DiskBackend;
