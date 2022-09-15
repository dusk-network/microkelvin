# microkelvin

[![Repository](https://img.shields.io/badge/github-microkelvin-blueviolet?logo=github)](https://github.com/dusk-network/microkelvin)
![Build Status](https://github.com/dusk-network/microkelvin/workflows/build/badge.svg)
[![Documentation](https://img.shields.io/badge/docs-microkelvin-blue?logo=rust)](https://docs.rs/microkelvin)

Crate for creating and traversing recursively annotated structures.

# Compound trait

```rust
/// A type that can recursively contain itself and leaves.
pub trait Compound<A>: Sized {
    /// The leaf type of the compound collection
    type Leaf;

    /// Returns a reference to a possible child at specified index
    fn child(&self, index: usize) -> Child<Self, A>;

    /// Returns a mutable reference to a possible child at specified index
    fn child_mut(&mut self, index: usize) -> ChildMut<Self, A>;
}
```

The `Compound` trait defines a type as a collection type. This means that it
can be searched and have branches constructed pointing to its elements.

# Branch walking 

The `Walker` trait can be implemented for walking the tree in a user defined
way. As an example, here's `AllLeaves` - an implementation used internally:

```rust
/// Walker that visits all leaves
pub struct AllLeaves;

impl<C, A> Walker<C, A> for AllLeaves
where
    C: Compound<A>,
{
    fn walk(&mut self, walk: Walk<C, A>) -> Step {
        for i in 0.. {
            match walk.child(i) {
                Child::Leaf(_) => return Step::Found(i),
                Child::Node(_) => return Step::Into(i),
                Child::Empty => (),
                Child::EndOfNode => return Step::Advance,
            }
        }
        unreachable!()
    }
}
```

# Usage

Please check out the [nstack](http://github.com/dusk-network/nstack)
implementation of a stack/vector type for a more advanced example.
