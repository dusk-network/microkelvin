// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.
//
// Copyright (c) DUSK NETWORK. All rights reserved.

use alloc::vec::Vec;

const VERSION: u8 = 0;

/// The size of the Id payload, used to store cryptographic hashes or inlined
/// values
pub const PAYLOAD_BYTES: usize = 32;

// We alias `IdHash` and `Inlined` versions of `Payload` to be able to use them
// interchangeably but with some type documentation

/// Type alias for an arbitrary Id payload, either a hash or an inlined value
pub type Payload = [u8; PAYLOAD_BYTES];
/// Type alias for a payload that is used as a hash
pub type IdHash = Payload;
/// Type alias for a payload that is used as an inlined value
pub type Inlined = Payload;

/// This is the Id type, that uniquely identifies slices of bytes,
/// in rust equivalent to `&[u8]`. As in the case with `&[u8]` the length is
/// also encoded in the type, making it a kind of a fat-pointer for content
/// addressed byte-slices.
///
/// The length of the corresponding byte-string is encoded in the first two
/// bytes in big endian.
///
/// If the length of the byteslice is less than or equal to 32 bytes, the bytes
/// are stored directly inline in the `bytes` field.
///
/// Proposal: The trailing bytes in an inlined value MUST be set to zero
#[derive(Hash, PartialEq, Eq, Default, Clone, Copy, Debug, PartialOrd, Ord)]
pub struct Id {
    version: u8,
    len: u32,
    payload: Payload,
}

impl Id {
    /// Creates a new Id from a type
    pub fn new<T>(t: &T) -> Self {
        // let len = t.encoded_len();
        // let payload = if len > PAYLOAD_BYTES {
        //     Store::put(&t.encode_to_vec())
        // } else {
        //     let mut stack_buf = Inlined::default();
        //     let mut sink = Sink::new(&mut stack_buf[..len]);
        //     t.encode(&mut sink);
        //     stack_buf
        // };

        // assert!(len <= u32::MAX as usize, "Payload length overflow");

        // Id {
        //     version: VERSION,
        //     len: (len as u32),
        //     payload,
        // }
        todo!()
    }

    /// Returns the computed hash of the value.
    ///
    /// Note that this is different from the payload itself in case of an
    /// inlined value, that normally does not get hashed.
    ///
    /// Useful for giving a well-distributed unique id for all `Canon` types,
    /// for use in hash maps for example.
    pub fn hash(&self) -> IdHash {
        // let len = self.size();
        // if len > PAYLOAD_BYTES {
        //     self.payload
        // } else {
        //     Store::hash(&self.payload[0..len])
        // }
        todo!()
    }

    /// Returns the bytes of the identifier
    pub fn payload(&self) -> &Payload {
        &self.payload
    }

    /// Consumes the Id and returns the payload bytes
    pub fn into_payload(self) -> [u8; PAYLOAD_BYTES] {
        self.payload
    }

    /// Returns the length of the represented data
    pub const fn size(&self) -> usize {
        self.len as usize
    }

    /// Attempts to reify the Id as an instance of type `T`
    pub fn reify<T>(&self) -> Result<T, ()> {
        todo!()
    }

    /// Takes the bytes corresponding to this id out of the underlying store.
    ///
    /// If the Id is inlined, this is a no-op and returns `Ok(None)`
    pub fn take_bytes(&self) -> Result<Option<Vec<u8>>, ()> {
        // if self.size() <= PAYLOAD_BYTES {
        //     Ok(None)
        // } else {
        //     Ok(Some(Store::take_bytes(self)?))
        // }
        todo!()
    }
}
