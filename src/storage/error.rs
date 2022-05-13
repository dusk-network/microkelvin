// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use std::io;
use thiserror::Error;

/// An error that can happen when persisting structures to disk
#[derive(Error, Debug)]
pub enum PersistError {
    /// An io-error occurred while persisting
    #[error(transparent)]
    Io(#[from] io::Error),
}
