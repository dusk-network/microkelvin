use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

/// Error resolving a merkle-link
#[derive(Debug, PartialEq, Archive, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub enum Error {
    /// Missing data
    Missing,
    /// Invalid data encountered     
    Invalid,
}
