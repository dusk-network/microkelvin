use crate::annotation::Annotation;

pub enum Traverse {
    Leaf(usize),
    Node(usize),
    None,
}

/// Trait for compound datastructures
pub trait Compound {
    type Leaf;
    type Annotation: Annotation<Self::Leaf>;

    fn annotation(&self) -> Self::Annotation;
    fn traverse<M: Annotation<Self::Leaf>>(&self, method: &mut M) -> Traverse;
}
