// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Microkelvin
//!
//! A library for dealing with tree-shaped Canonical data. It has three parts:
//!
//! `Compound`, a trait for a generic way to implement tree structures
//! `Annotation`, a trait for annotated subtrees used for searching
//! `Branch` and `BranchMut`, types for representing branches in tree-formed
//! data as well as methods of search.

// #![cfg_attr(not(feature = "persistence"), no_std)]
#![deny(missing_docs)]

#[macro_use]
extern crate alloc;

mod annotations;
mod branch;
mod branch_mut;
mod compound;
mod generic;
mod link;
mod walk;

#[cfg(feature = "persistence")]
mod persist;

pub use annotations::{
    Annotation, Cardinality, Combine, GetMaxKey, Keyed, MaxKey, Nth,
};
pub use branch::Branch;
pub use branch_mut::BranchMut;

pub use compound::{AnnoIter, Child, ChildMut, Compound, MutableLeaves};
pub use generic::{GenericAnnotation, GenericChild, GenericLeaf, GenericTree};
pub use link::{Link, LinkAnnotation, LinkCompound, LinkCompoundMut};
pub use walk::{First, Step, Walk, Walker};

#[cfg(feature = "persistence")]
pub use persist::{
    Backend, BackendCtor, DiskBackend, PersistError, PersistedId, Persistence,
    PutResult,
};
