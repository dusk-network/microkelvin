// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use thiserror::Error;
use std::io;

/// An error that can happen when persisting structures to disk
#[derive(Error, Debug)]
pub enum PersistError {
    /// An io-error occured while persisting
    #[error(transparent)]
    Io(#[from] io::Error),
    /// No backend found
    #[error("Backend not found")]
    BackendUnavailable,
    // todo
    // Serialisation error occurred while persisting
    // #[error("Serialisation error: {0:?}")]
    // Canon(CanonError),
    // todo
    // Other backend specific error
    //#[error(transparent)]
    //Other(#[from] anyhow::Error),
}
