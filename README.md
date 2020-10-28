# microkelvin

Library for creating and searching through annotated merkle trees.

Built on the [`canonical`](http://github.com/dusk-network/canonical) serialization and FFI library.

# Compound trait

```rust
/// Trait for compound datastructures
pub trait Compound<S>
where
    Self: Canon<S>,
    S: Store,
{
    /// The leaf type of the collection
    type Leaf;

    /// The annotation type of the connection
    type Annotation;

    /// Returns a reference to a possible child at specified offset
    fn child(&self, ofs: usize) -> Child<Self, S>;

    /// Returns a mutable reference to a possible child at specified offset
    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, S>;
}
```

The Compound trait defines a type as a collection type. This means that it can be searched and have branches constructed pointing to its elements.

# Annotation trait

The annotation trait keeps annotations of subtrees, for example total leaf amount (`Cardinality` in reference implementation), or which leaf compares the greatest (`Max` in reference implementation)

# Branch walking

This is ane example of walking a recursive structure to yieald a branch pointing at a specific leaf.

It i implemented on any type implementing `Compound` whose annotation can be borrowed as `Cardinality`. Giving this capability to any such structure.

```rust
impl<'a, C, S> Nth<'a, S> for C
where
    C: Compound<S>,
    C::Annotation: Annotation<C, S> + Borrow<Cardinality>,
    S: Store,
{
    fn nth<const N: usize>(
        &'a self,
        mut index: u64,
    ) -> Result<Option<Branch<'a, Self, S, N>>, S::Error> {
        Branch::walk(self, |f| match f {
            Walk::Leaf(l) => {
                if index == 0 {
                    Step::Found(l)
                } else {
                    index -= 1;
                    Step::Next
                }
            }
            Walk::Node(n) => {
                let &Cardinality(card) = n.annotation().borrow();
                if card <= index {
                    index -= card;
                    Step::Next
                } else {
                    Step::Into(n)
                }
            }
        })
    }
		// [ ... ]
}
```
# usage

Please check out the [`nstack`](http://github.com/dusk-network/nstack) implementation of a stack/vector type for a more advanced example.