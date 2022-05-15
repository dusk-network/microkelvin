use core::borrow::Borrow;
use core::fmt::Debug;
use core::mem;

use bytecheck::CheckBytes;
use rkyv::{Archive, Deserialize, Serialize};

use crate::tower::{Fundamental, WellArchived, WellFormed};
use crate::ArchivedChild;
use crate::ArchivedCompound;
use crate::Keyed;
use crate::TreeViz;
use crate::{Annotation, Child, ChildMut, Compound, MaxKey};

use super::leafnode::LeafNode;
use super::linknode::LinkNode;

/// A BTree key-value pair
#[derive(Archive, Clone, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct Pair<K, V> {
    /// The key of the key-value pair
    pub k: K,
    /// The value of the key-value pair
    pub v: V,
}

impl<K, V> Debug for Pair<K, V>
where
    K: Debug,
    V: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}: {:?}", &self.k, &self.v)
    }
}

impl<K, V> Keyed<K> for Pair<K, V> {
    fn key(&self) -> &K {
        &self.k
    }
}

// A BTreeMap
#[derive(Clone, Deserialize, Archive, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub struct BTreeMap<K, V, A, const LE: usize = 3, const LI: usize = 3>(
    pub(crate) BTreeMapInner<K, V, A, LE, LI>,
);

impl<K, V, A, const LE: usize, const LI: usize> Debug
    for BTreeMap<K, V, A, LE, LI>
where
    K: Fundamental + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Annotation<Pair<K, V>> + Fundamental + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.treeify(f, 0)
    }
}

impl<K, V, A, const LE: usize, const LI: usize> Default
    for BTreeMap<K, V, A, LE, LI>
{
    fn default() -> Self {
        BTreeMap(BTreeMapInner::LeafNode(Default::default()))
    }
}

/// We have an inner type to avoid having to make LeafNode and LinkNode public
/// TODO make this private.
#[derive(Archive, Clone, Deserialize, Serialize)]
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
                (BTreeMapInner::LinkNode(a), BTreeMapInner::LinkNode(b)) => {
                    let linknode = LinkNode::from_link_nodes(mem::take(a), b);
                    *self = BTreeMap(BTreeMapInner::LinkNode(linknode));
                    None
                }
                _ => unreachable!(),
            },
        }
    }

    /// Get a reference to the value of the key `k`, if any
    pub fn get<O>(&self, k: &O) -> Option<&V>
    where
        K: Borrow<O>,
        O: Ord + Debug,
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
        O: Ord + Debug,
    {
        match self.sub_remove(o) {
            Remove::None => None,
            Remove::Removed(v) => Some(v),
            Remove::Underflow(v) => {
                println!("underflow toplevel\n--\n{:?}", self);
                match &mut self.0 {
                    BTreeMapInner::LeafNode(_) => Some(v),
                    BTreeMapInner::LinkNode(links) => {
                        let mut taken = mem::take(links);
                        *self = taken.remove_link(0).into_inner();
                        Some(v)
                    }
                }
            }
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
        O: Ord + Debug,
    {
        match &mut self.0 {
            BTreeMapInner::LeafNode(leaves) => leaves.remove(o),
            BTreeMapInner::LinkNode(links) => links.remove(o),
        }
    }

    pub(crate) fn prepend(&mut self, affix: Self) -> Result<(), ()> {
        match (&mut self.0, &mut affix.0) {
            (BTreeMapInner::LeafNode(a), BTreeMapInner::LeafNode(b)) => {
                a.prepend(b)
            }
            (BTreeMapInner::LinkNode(a), BTreeMapInner::LinkNode(b)) => {
                a.prepend(b)
            }
            _ => unreachable!(),
        }
    }

    // Function used in tests to enforce invariants below

    #[doc(hidden)]
    pub fn correct_empty_state(&self) -> bool {
        match &self.0 {
            BTreeMapInner::LeafNode(leaves) => leaves.len() == 0,
            _ => false,
        }
    }

    #[doc(hidden)]
    pub fn all_leaves_at_same_level(&self) -> bool {
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

    #[doc(hidden)]
    // for use only in tests
    pub fn n_leaves(&self) -> u32 {
        use crate::All;

        let mut leaves = 0;

        if let Some(branch) = self.walk(All) {
            for _ in branch {
                leaves += 1;
            }
        }

        leaves
    }
}
