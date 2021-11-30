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
mod walk;
mod wrappers;

pub use annotations::{
    ARef, Annotation, Cardinality, Combine, FindMaxKey, Keyed, MaxKey, Member,
    Nth,
};
pub use branch::{Branch, BranchRef, MappedBranch};
pub use branch_mut::{BranchMut, MappedBranchMut};
pub use compound::{
    ArchivedChild, ArchivedCompound, Child, ChildMut, Compound, MutableLeaves,
};
pub use link::{ArchivedLink, Link};
pub use walk::{All, Discriminant, Step, Walkable, Walker};
pub use wrappers::{MaybeArchived, MaybeStored, Primitive};

mod storage;
pub use storage::{Storage, Store, Stored};

#[cfg(feature = "host")]
pub use storage::HostStore;
