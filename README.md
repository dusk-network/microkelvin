# microkelvin

Library for creating and searching through annotated merkle trees.

Built on the [`canonical`](http://github.com/dusk-network/canonical) serialization and FFI library.

# Compound trait

```rust
/// A type that can recursively contain itself and leaves.
pub trait Compound<A>: Sized + Canon {
    /// The leaf type of the Compound collection
    type Leaf;

    /// Returns a reference to a possible child at specified offset
    fn child(&self, ofs: usize) -> Child<Self, A>
    where
        A: Annotation<Self::Leaf>;

    /// Returns a mutable reference to a possible child at specified offset
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>
    where
        A: Annotation<Self::Leaf>;
```

The Compound trait defines a type as a collection type. This means that it can be searched and have branches constructed pointing to its elements.

# Annotation/Combine trait

```rust
/// The trait defining an annotation type over a leaf
pub trait Annotation<Leaf>: Default + Clone {
    /// Creates an annotation from the leaf type
    fn from_leaf(leaf: &Leaf) -> Self;
}
```

```rust
/// Trait for defining how to combine Annotations
pub trait Combine<C, A>: Annotation<C::Leaf>
where
    C: Compound<A>,
{
    /// Combines multiple annotations
    fn combine(node: &C) -> Self;
}
```

The annotation and combine traits keep an automatically updated annotation of subtrees, for example total leaf amount (`Cardinality` in reference implementation), or which leaf compares the greatest (`Max` in reference implementation)

# Branch walking

This is ane example of walking a recursive structure to yield a branch pointing at the nth leaf of the collection, if any.

It is automatically implemented on all types implementing `Compound` whose annotation can be borrowed as `Cardinality`. Giving this capability to any such structure.

```rust

impl<'a, C, A> Nth<'a, A> for C
where
    C: Compound<A> + Clone,
    A: Annotation<Self::Leaf> + Borrow<Cardinality>,
{
    fn nth(
        &'a self,
        mut remainder: u64,
    ) -> Result<Option<Branch<'a, Self, A>>, CanonError> {
        // Return the first that satisfies the walk
        Branch::<_, A>::walk(self, |w| nth(w, &mut remainder))
    }

    fn nth_mut(
        &'a mut self,
        mut remainder: u64,
    ) -> Result<Option<BranchMut<'a, Self, A>>, CanonError> {
        // Return the first mutable branch that satisfies the walk
        BranchMut::<_, A>::walk(self, |w| nth(w, &mut remainder))
    }
}
```
# usage

Please check out the [`nstack`](http://github.com/dusk-network/nstack) implementation of a stack/vector type for a more advanced example.
