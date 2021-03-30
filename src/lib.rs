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

#![no_std]
#![deny(missing_docs)]

#[macro_use]
extern crate alloc;

mod annotations;
mod branch;
mod branch_mut;
mod compound;
mod walk;

pub use annotations::{
    Ann, Annotated, Annotation, Cardinality, Keyed, Max, Nth,
};
pub use branch::Branch;
pub use branch_mut::BranchMut;
pub use compound::{Child, ChildMut, Compound};
pub use walk::{Step, Walk};
