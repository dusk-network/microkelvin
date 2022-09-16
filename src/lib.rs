// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

//! Crate for creating and traversing recursively annotated structures. It has three
//! parts:
//!
//! - [`Compound`], a trait for a generic way to implement tree structures.
//! - [`Branch`] and [`BranchMut`], types for representing branches in tree-formed
//! data as well as methods for searching.
//! - [`Walker`], a trait for a generic way of walking [`Compound`]s.

#![no_std]
#![deny(missing_docs)]
#![deny(clippy::all)]

#[macro_use]
extern crate alloc;

mod branch;
mod branch_mut;
mod compound;
mod walk;

pub use branch::Branch;
pub use branch_mut::BranchMut;

pub use compound::{Child, ChildMut, Compound, MutableLeaves};
pub use walk::{First, Step, Walk, Walker};
