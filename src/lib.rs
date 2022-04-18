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
#![cfg_attr(not(feature = "host"), no_std)]
// #![deny(missing_docs)]

#[macro_use]
extern crate alloc;

mod annotations;
mod branch;
mod branch_mut;
mod compound;
mod link;
mod tower;
mod viz;
mod walk;
mod wrappers;

/// Collections implemented using microkelvin
pub mod collections;

pub use annotations::{
    ARef, Annotation, Cardinality, Combine, FindMaxKey, Keyed, MaxKey, Member,
    Nth,
};
pub use branch::{Branch, BranchRef, MappedBranch};
pub use branch_mut::{BranchMut, BranchRefMut, MappedBranchMut};
pub use compound::{
    ArchivedChild, ArchivedCompound, Child, ChildMut, Compound, MutableLeaves,
};
pub use link::{ArchivedLink, Link};
pub use tower::{Fundamental, WellArchived, WellFormed};
pub use walk::{All, Discriminant, Step, Walkable, Walker};
pub use wrappers::{MaybeArchived, MaybeStored};

mod storage;
pub use storage::{
    Ident, Store, StoreProvider, StoreRef, StoreSerializer, Stored, Token,
    TokenBuffer,
};

pub use viz::TreeViz;

#[cfg(feature = "host")]
pub use storage::{HostStore, UnwrapInfallible};
