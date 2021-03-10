use canonical::Canon;
use canonical_derive::Canon;
use microkelvin::{Annotated, Annotation, Child, ChildMut, Compound};

#[derive(Clone, Canon, Debug)]
enum LinkedList<T, A> {
    Empty,
    Node { val: T, next: Annotated<Self, A> },
}

impl<T, A> Default for LinkedList<T, A> {
    fn default() -> Self {
        LinkedList::Empty
    }
}

impl<T, A> Compound<A> for LinkedList<T, A>
where
    T: Canon,
    A: Canon,
{
    type Leaf = T;

    fn child(&self, ofs: usize) -> Child<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => Child::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => Child::Node(next),
            (LinkedList::Node { .. }, _) => Child::EndOfNode,
            (LinkedList::Empty, _) => Child::EndOfNode,
        }
    }

    fn child_mut(&mut self, ofs: usize) -> ChildMut<Self, A>
    where
        A: Annotation<Self::Leaf>,
    {
        match (self, ofs) {
            (LinkedList::Node { val, .. }, 0) => ChildMut::Leaf(val),
            (LinkedList::Node { next, .. }, 1) => ChildMut::Node(next),
            (LinkedList::Node { .. }, _) => ChildMut::EndOfNode,
            (LinkedList::Empty, _) => ChildMut::EndOfNode,
        }
    }
}

impl<T, A> LinkedList<T, A>
where
    Self: Compound<A>,
    A: Annotation<<Self as Compound<A>>::Leaf>,
{
    fn new() -> Self {
        Default::default()
    }

    fn insert(&mut self, t: T) {
        match core::mem::take(self) {
            LinkedList::Empty => {
                *self = LinkedList::Node {
                    val: t,
                    next: Annotated::new(LinkedList::Empty),
                }
            }
            old @ LinkedList::Node { .. } => {
                *self = LinkedList::Node {
                    val: t,
                    next: Annotated::new(old),
                };
            }
        }
    }
}

#[test]
fn insert_nth() {
    let n: u64 = 1024;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.insert(i)
    }

    for i in 0..n {
        assert_eq!(*list.nth(i).unwrap().unwrap(), n - i - 1)
    }
}

#[test]
fn insert_mut() {
    let n: u64 = 1024;

    use microkelvin::{Cardinality, Nth};

    let mut list = LinkedList::<_, Cardinality>::new();

    for i in 0..n {
        list.insert(i)
    }

    for i in 0..n {
        println!("mutating position {}", i);
        *list.nth_mut(i).unwrap().unwrap() += 1;
    }

    for i in 0..n {
        assert_eq!(*list.nth(i).unwrap().unwrap(), n - i - 1)
    }
}
