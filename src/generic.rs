use alloc::vec::Vec;

use canonical::{Canon, CanonError, EncodeToVec, Id, Source};
use canonical_derive::Canon;

use crate::link::Link;
use crate::{Annotation, Compound};

const TAG_EMPTY: u8 = 0;
const TAG_LEAF: u8 = 1;
const TAG_LINK: u8 = 2;

/// A generic annotation
#[derive(Clone, Canon, Debug)]
pub struct GenericAnnotation(Vec<u8>);

/// A generic leaf
#[derive(Clone, Canon, Debug)]
pub struct GenericLeaf(Vec<u8>);

impl GenericLeaf {
    pub(crate) fn new<C: Canon>(c: &C) -> Self {
        GenericLeaf(c.encode_to_vec())
    }

    /// Cast the generic leaf to a concrete type
    pub fn cast<T: Canon>(&self) -> Result<T, CanonError> {
        T::decode(&mut Source::new(&self.0))
    }
}

impl GenericAnnotation {
    pub(crate) fn new<A: Canon>(a: &A) -> Self {
        GenericAnnotation(a.encode_to_vec())
    }

    /// Cast the generic leaf to a concrete type
    pub fn cast<A: Canon>(&self) -> Result<A, CanonError> {
        A::decode(&mut Source::new(&self.0))
    }
}

/// A generic child of a collection
#[derive(Clone, Debug)]
pub enum GenericChild {
    /// Child is empty
    Empty,
    /// Child is a leaf    
    Leaf(GenericLeaf),
    /// Child is a link        
    Link(Id, GenericAnnotation),
}

impl Canon for GenericChild {
    fn encode(&self, sink: &mut canonical::Sink) {
        match self {
            Self::Empty => TAG_EMPTY.encode(sink),
            Self::Leaf(leaf) => {
                TAG_LEAF.encode(sink);
                leaf.encode(sink)
            }
            Self::Link(id, annotation) => {
                TAG_LINK.encode(sink);
                id.encode(sink);
                annotation.encode(sink);
            }
        }
    }

    fn decode(source: &mut canonical::Source) -> Result<Self, CanonError> {
        match u8::decode(source)? {
            TAG_EMPTY => Ok(GenericChild::Empty),
            TAG_LEAF => Ok(GenericChild::Leaf(GenericLeaf::decode(source)?)),
            TAG_LINK => {
                let id = Id::decode(source)?;
                let anno = GenericAnnotation::decode(source)?;
                Ok(GenericChild::Link(id, anno))
            }
            _ => Err(CanonError::InvalidEncoding),
        }
    }

    fn encoded_len(&self) -> usize {
        let tag_len = 1;
        match self {
            Self::Empty => tag_len,
            Self::Leaf(leaf) => {
                let size_len = 2;
                tag_len + size_len + leaf.encoded_len()
            }
            Self::Link(id, anno) => {
                tag_len + id.encoded_len() + anno.encoded_len()
            }
        }
    }
}

#[derive(Default, Clone, Canon, Debug)]
pub struct GenericTree(Vec<GenericChild>);

impl GenericTree {
    pub(crate) fn new() -> Self {
        GenericTree(vec![])
    }

    pub(crate) fn push_empty(&mut self) {
        self.0.push(GenericChild::Empty)
    }

    pub(crate) fn push_leaf<L: Canon>(&mut self, leaf: &L) {
        self.0.push(GenericChild::Leaf(GenericLeaf::new(leaf)))
    }

    pub(crate) fn push_link<C, A>(&mut self, link: &Link<C, A>)
    where
        C: Compound<A>,
        C::Leaf: Canon,
        A: Annotation<C::Leaf> + Canon,
    {
        let id = link.id();
        let anno = GenericAnnotation::new(&*link.annotation());
        self.0.push(GenericChild::Link(id, anno));
    }

    pub fn children(&self) -> &[GenericChild] {
        &self.0
    }
}