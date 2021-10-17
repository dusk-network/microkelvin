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

#![deny(missing_docs)]

#[macro_use]
extern crate alloc;

mod annotations;
mod branch;
mod branch_mut;
mod compound;
mod link;
mod primitive;
mod walk;

pub use annotations::{
    ARef, Annotation, Cardinality, Combine, GetMaxKey, Keyed, MaxKey, Nth,
};
pub use branch::Branch;
pub use branch_mut::BranchMut;
pub use compound::{
    AnnoIter, ArchivedChild, ArchivedCompound, Child, ChildMut, Compound,
    MutableLeaves,
};
pub use link::Link;
pub use primitive::Primitive;
pub use walk::{AllLeaves, First, Step, Walker};

mod storage;
pub use storage::{Portal, PortalDeserializer, Storage, StorageSerializer};
