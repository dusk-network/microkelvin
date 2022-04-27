use core::borrow::Borrow;
use core::fmt::Debug;
use core::mem;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use crate::tower::{Fundamental, WellArchived, WellFormed};
use crate::ArchivedChild;
use crate::ArchivedCompound;
use crate::Keyed;
use crate::{Annotation, Child, ChildMut, Compound, Link, MaxKey};

use super::leafnode::LeafNode;
use super::linknode::Append;
use super::linknode::LinkNode;

/// A BTree key-value pair
#[derive(Archive, Clone, Serialize, Deserialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub struct Pair<K, V> {
    /// The key of the key-value pair
    pub k: K,
    /// The value of the key-value pair
    pub v: V,
}

impl<K, V> Keyed<K> for Pair<K, V> {
    fn key(&self) -> &K {
        &self.k
    }
}

// A BTreeMap
#[derive(Clone, Deserialize, Archive, Serialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub struct BTreeMap<K, V, A, const LE: usize = 3, const LI: usize = 3>(
    pub(crate) BTreeMapInner<K, V, A, LE, LI>,
);

impl<K, V, A, const LE: usize, const LI: usize> Default
    for BTreeMap<K, V, A, LE, LI>
{
    fn default() -> Self {
        BTreeMap(BTreeMapInner::LeafNode(Default::default()))
    }
}

/// We have an inner type to avoid having to make LeafNode and LinkNode public
/// TODO make this private.
#[derive(Archive, Clone, Deserialize, Serialize, Debug)]
#[archive_attr(derive(CheckBytes))]
pub enum BTreeMapInner<K, V, A, const LE: usize, const LI: usize> {
    /// A node of leaves
    LeafNode(LeafNode<K, V, LE>),
    /// A node of links
    LinkNode(LinkNode<K, V, A, LE, LI>),
}

impl<K, V, A, const LE: usize, const LI: usize> From<LeafNode<K, V, LE>>
    for BTreeMap<K, V, A, LE, LI>
{
    fn from(le: LeafNode<K, V, LE>) -> Self {
        BTreeMap(BTreeMapInner::LeafNode(le))
    }
}

impl<K, V, A, const LE: usize, const LI: usize> From<LinkNode<K, V, A, LE, LI>>
    for BTreeMap<K, V, A, LE, LI>
{
    fn from(li: LinkNode<K, V, A, LE, LI>) -> Self {
        BTreeMap(BTreeMapInner::LinkNode(li))
    }
}

impl<K, V, A, const LE: usize, const LI: usize> Compound<A>
    for BTreeMap<K, V, A, LE, LI>
where
    K: Fundamental + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Annotation<Pair<K, V>> + Fundamental + Debug,
{
    type Leaf = Pair<K, V>;

    fn child(&self, ofs: usize) -> Child<Self, A> {
        match &self.0 {
            BTreeMapInner::LeafNode(le) => match le.get_leaf(ofs) {
                Some(pair) => Child::Leaf(pair),
                None => Child::End,
            },
            BTreeMapInner::LinkNode(li) => match li.get_link(ofs) {
                Some(link) => Child::Link(link),
                None => Child::End,
            },
        }
    }

    fn child_mut(&mut self, _ofs: usize) -> ChildMut<Self, A> {
        todo!()
    }
}

impl<K, V, A, const LE: usize, const LI: usize>
    ArchivedCompound<BTreeMap<K, V, A, LE, LI>, A>
    for ArchivedBTreeMap<K, V, A, LE, LI>
where
    K: Fundamental + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Annotation<Pair<K, V>> + Fundamental + Debug,
{
    fn child(
        &self,
        _ofs: usize,
    ) -> ArchivedChild<BTreeMap<K, V, A, LE, LI>, A> {
        todo!()
    }
}

#[derive(Debug)]
pub(crate) enum Insert<V, S> {
    Ok,
    Replaced(V),
    Split(S),
}

#[derive(Debug)]
pub(crate) enum Remove<V> {
    None,
    Removed(V),
    Underflow(V),
}

impl<K, V, A, const LE: usize, const LI: usize> BTreeMap<K, V, A, LE, LI>
where
    K: Fundamental + Ord + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Fundamental + Annotation<Pair<K, V>> + Borrow<MaxKey<K>> + Debug,
    A::Archived: Debug,
{
    /// Create a new empty BTreemap
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a key-value pair into the map, returning the replaced value if
    /// any.
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        match self.sub_insert(k, v) {
            Insert::Ok => None,
            Insert::Replaced(v) => Some(v),
            Insert::Split(s) => match (&mut self.0, s.0) {
                (BTreeMapInner::LeafNode(a), BTreeMapInner::LeafNode(b)) => {
                    let linknode = LinkNode::from_leaf_nodes(mem::take(a), b);

                    *self = BTreeMap(BTreeMapInner::LinkNode(linknode));
                    None
                }
                (BTreeMapInner::LeafNode(_), BTreeMapInner::LinkNode(_)) => {
                    todo!()
                }
                (BTreeMapInner::LinkNode(a), BTreeMapInner::LeafNode(b)) => {
                    let link = Link::new(BTreeMap(BTreeMapInner::LeafNode(b)));

                    match a.append_link(link) {
                        Append::Ok => None,
                        Append::Split(_s) => todo!(),
                    }
                }
                (BTreeMapInner::LinkNode(_), BTreeMapInner::LinkNode(_)) => {
                    todo!()
                }
            },
        }
    }

    /// Get a reference to the value of the key `k`, if any
    pub fn get<O>(&self, k: &O) -> Option<&V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        match &self.0 {
            BTreeMapInner::LeafNode(leaves) => leaves.get(k),
            BTreeMapInner::LinkNode(links) => links.get(k),
        }
    }

    /// Remove the value of key `k`, returning it if present
    /// Get a reference to the value of the key `k`, if any
    pub fn remove<O>(&mut self, o: &O) -> Option<V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        match self.sub_remove(o) {
            Remove::None => None,
            Remove::Removed(v) => Some(v),
            Remove::Underflow(v) => {
                println!("underflow toplevel");

                match &mut self.0 {
                    BTreeMapInner::LeafNode(_) => Some(v),
                    BTreeMapInner::LinkNode(links) => {
                        debug_assert!(links.len() == 1);
                        let mut taken = mem::take(links);
                        *self = taken.remove_link(0).into_inner();
                        Some(v)
                    }
                }
            }
        }
    }

    fn len(&self) -> usize {
        match &self.0 {
            BTreeMapInner::LeafNode(le) => le.len(),
            BTreeMapInner::LinkNode(li) => li.len(),
        }
    }

    fn sub_insert(&mut self, k: K, v: V) -> Insert<V, Self> {
        match &mut self.0 {
            BTreeMapInner::LeafNode(leaves) => match leaves.insert_leaf(k, v) {
                Insert::Ok => Insert::Ok,
                Insert::Replaced(v) => Insert::Replaced(v),
                Insert::Split(s) => {
                    Insert::Split(BTreeMap(BTreeMapInner::LeafNode(s)))
                }
            },
            BTreeMapInner::LinkNode(links) => match links.insert_leaf(k, v) {
                Insert::Ok => Insert::Ok,
                Insert::Replaced(v) => Insert::Replaced(v),
                Insert::Split(linknode) => {
                    Insert::Split(BTreeMap(BTreeMapInner::LinkNode(linknode)))
                }
            },
        }
    }

    pub(crate) fn sub_remove<O>(&mut self, o: &O) -> Remove<V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        match &mut self.0 {
            BTreeMapInner::LeafNode(leaves) => leaves.remove_leaf(o),
            BTreeMapInner::LinkNode(links) => links.remove(o),
        }
    }

    #[cfg(test)]
    fn correct_empty_state(&self) -> bool {
        match &self.0 {
            BTreeMapInner::LeafNode(leaves) => leaves.len() == 0,
            _ => false,
        }
    }

    #[cfg(test)]
    fn all_leaves_at_same_level(&self) -> bool {
        use crate::All;

        match self.walk(All) {
            Some(branch) => {
                let first_count = branch.depth();
                let mut iter = branch.into_iter();

                loop {
                    match iter.next() {
                        Some(_) => {
                            if first_count != iter.depth() {
                                return false;
                            }
                        }
                        None => return true,
                    }
                }
            }
            None => true,
        }
    }
}

#[cfg(test)]
mod test {
    use crate::{MaxKey, TreeViz};

    use super::BTreeMap;

    use rkyv::rend::LittleEndian;

    const S: i32 = 4;
    const N: i32 = 16;

    #[test]
    fn btree_add_remove_simple() {
        let mut map =
            BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new(
            );

        for o in S..N {
            println!("\n------------\nTESTING N = {}", o);

            for i in 0..o {
                println!("insert {:?}", i);
                assert_eq!(map.insert(LittleEndian::from(i), i), None);

                map.print_tree();

                assert!(map.all_leaves_at_same_level());
            }

            for i in 0..o {
                println!("removing {:?}", i);

                assert_eq!(map.remove(&LittleEndian::from(i)), Some(i));

                map.print_tree();

                assert!(map.all_leaves_at_same_level());
            }

            assert!(map.correct_empty_state());
        }
    }

    #[test]
    fn btree_add_remove_reverse() {
        let mut map =
            BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new(
            );

        for o in S..N {
            for i in 0..o {
                let i = o - i - 1;
                assert_eq!(map.insert(LittleEndian::from(i), i), None);

                assert!(map.all_leaves_at_same_level());
            }

            for i in 0..o {
                let i = o - i - 1;
                assert_eq!(map.remove(&LittleEndian::from(i)), Some(i));
                println!("removed {}", i);
                map.print_tree();

                assert!(map.all_leaves_at_same_level());
            }
        }

        assert!(map.correct_empty_state());
    }

    #[test]
    fn btree_add_change_remove() {
        let mut map =
            BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new(
            );

        for o in S..N {
            println!("\n------------\nTESTING N = {}", o);

            for i in 0..o {
                println!("insert {:?}", i);
                assert_eq!(map.insert(LittleEndian::from(i), i), None);

                map.print_tree();

                assert!(map.all_leaves_at_same_level());
            }

            for i in 0..o {
                println!("re-insert {:?}", i);
                assert_eq!(map.insert(LittleEndian::from(i), i + 1), Some(i));

                map.print_tree();
            }

            for i in 0..o {
                assert_eq!(map.get(&LittleEndian::from(i)), Some(&(i + 1)));
            }

            for i in 0..o {
                println!("removing {:?}", i);

                assert_eq!(map.remove(&LittleEndian::from(i)), Some(i + 1));

                map.print_tree();

                assert!(map.all_leaves_at_same_level());
            }

            assert!(map.correct_empty_state());
        }
    }
}
