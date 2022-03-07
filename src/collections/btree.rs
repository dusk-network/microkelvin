use core::borrow::Borrow;
use core::borrow::BorrowMut;
use core::cmp::Ordering;
use core::mem;

use bytecheck::CheckBytes;
use rkyv::ser::{ScratchSpace, Serializer};
use rkyv::{Archive, Deserialize, Serialize};

use crate::tower::{Fundamental, WellArchived, WellFormed};
use crate::Keyed;
use crate::{
    Annotation, Child, ChildMut, Compound, Link, MaxKey, StoreProvider,
    StoreSerializer,
};

/// A BTree key-value pair
#[derive(Archive, Clone, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct Pair<K, V> {
    /// The key of the key-value pair
    pub key: K,
    /// The value of the key-value pair
    pub val: V,
}

impl<K, V> Keyed<K> for Pair<K, V> {
    fn key(&self) -> &K {
        &self.key
    }
}

impl<K, V> Pair<K, V> {
    fn into_val(self) -> V {
        self.val
    }
}

/// TODO make not public.
#[derive(Archive, Clone, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
pub struct LeafNode<K, V, const LE: usize>(Vec<Pair<K, V>>);

#[derive(Archive, Clone, Serialize, Deserialize)]
#[archive_attr(derive(CheckBytes))]
#[archive(bound(serialize = "
  K: Fundamental,
  V: WellFormed,
  V::Archived: WellArchived<V>,
  A: Fundamental + Annotation<Pair<K, V>>,
  __S: Sized + Serializer + BorrowMut<StoreSerializer> + ScratchSpace"))]
#[archive(bound(deserialize = "
  A: Fundamental,
  __D: StoreProvider,"))]
/// TODO make not public.
pub struct LinkNode<K, V, A, const LE: usize, const LI: usize>(
    #[omit_bounds] Vec<Link<BTreeMap<K, V, A, LE, LI>, A>>,
);

/// A BTreeMap
#[derive(Clone, Deserialize, Archive, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub struct BTreeMap<K, V, A, const LE: usize = 3, const LI: usize = 3>(
    BTreeMapInner<K, V, A, LE, LI>,
);

impl<K, V, A> Default for BTreeMap<K, V, A> {
    fn default() -> Self {
        Self(Default::default())
    }
}

/// We have an inner type to avoid having to make LeafNode and LinkNode public
/// TODO make this not public.
#[derive(Archive, Clone, Deserialize, Serialize)]
#[archive_attr(derive(CheckBytes))]
pub enum BTreeMapInner<K, V, A, const LE: usize, const LI: usize> {
    /// A node of leaves
    LeafNode(LeafNode<K, V, LE>),
    /// A node of links
    LinkNode(LinkNode<K, V, A, LE, LI>),
}

impl<K, V, A, const LE: usize, const LI: usize> Default
    for BTreeMapInner<K, V, A, LE, LI>
{
    fn default() -> Self {
        BTreeMapInner::LeafNode(LeafNode::default())
    }
}

impl<K, V, const N: usize> Default for LeafNode<K, V, N> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K, V, const LE: usize> LeafNode<K, V, LE>
where
    K: Fundamental + Ord,
{
    #[inline(always)]
    fn split_point() -> usize {
        (LE + 1) / 2
    }

    #[inline(always)]
    fn full(&self) -> bool {
        self.0.len() == LE
    }

    pub fn insert(&mut self, k: K, v: V) -> Result<Option<V>, Self> {
        match self
            .0
            .binary_search_by(|Pair { key, .. }| -> Ordering { key.cmp(&k) })
        {
            Ok(idx) => Ok(Some(mem::replace(&mut self.0[idx].val, v))),
            Err(idx) => {
                if self.full() {
                    let point = Self::split_point();
                    println!("point {:?}", point);
                    let mut rest = self.0.split_off(point);

                    match rest[0].key.cmp(&k) {
                        Ordering::Greater => {
                            self.0.insert(idx, Pair { key: k, val: v });
                        }
                        Ordering::Equal => unreachable!(
                            "Split on equal key instead of replacing"
                        ),
                        Ordering::Less => {
                            rest.insert(idx - point, Pair { key: k, val: v })
                        }
                    }
                    Err(LeafNode(rest))
                } else {
                    self.0.insert(idx, Pair { key: k, val: v });
                    Ok(None)
                }
            }
        }
    }

    pub fn get<O>(&self, k: &O) -> Option<&V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        if let Ok(idx) =
            self.0.binary_search_by(|Pair { key, .. }| -> Ordering {
                key.borrow().cmp(k)
            })
        {
            Some(&self.0[idx].val)
        } else {
            None
        }
    }

    pub fn remove<O>(&mut self, k: &O) -> Option<V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        if let Ok(idx) =
            self.0.binary_search_by(|Pair { key, .. }| -> Ordering {
                key.borrow().cmp(k)
            })
        {
            Some(self.0.remove(idx).into_val())
        } else {
            None
        }
    }
}

impl<K, V, A, const LE: usize, const LI: usize> LinkNode<K, V, A, LE, LI>
where
    K: Fundamental + Ord,
    V: WellFormed,
    V::Archived: WellArchived<V>,
    A: Fundamental + Annotation<Pair<K, V>> + Borrow<MaxKey<K>>,
{
    fn from_leaf_nodes(a: LeafNode<K, V, LE>, b: LeafNode<K, V, LE>) -> Self {
        let map_a = BTreeMap(BTreeMapInner::LeafNode(a));
        let map_b = BTreeMap(BTreeMapInner::LeafNode(b));
        let link_a = Link::new(map_a);
        let link_b = Link::new(map_b);
        LinkNode(vec![link_a, link_b])
    }

    pub fn get<O>(&self, k: &O) -> Option<&V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        if let Ok(idx) = self.0.binary_search_by(|link| -> Ordering {
            match (*link.annotation()).borrow() {
                MaxKey::NegativeInfinity => unreachable!(),
                MaxKey::Maximum(m) => m.borrow().cmp(k),
            }
        }) {
            todo!()
        } else {
            None
        }
    }
}

impl<K, V, A, const LE: usize, const LI: usize> Compound<A>
    for BTreeMap<K, V, A, LE, LI>
where
    K: Fundamental,
    V: WellFormed,
    V::Archived: WellArchived<V>,
    A: Annotation<Pair<K, V>>,
{
    type Leaf = Pair<K, V>;

    fn child(&self, ofs: usize) -> Child<Self, A> {
        match &self.0 {
            BTreeMapInner::LeafNode(leaves) => match leaves.0.get(ofs) {
                Some(leaf) => Child::Leaf(leaf),
                None => Child::End,
            },
            BTreeMapInner::LinkNode(links) => match links.0.get(ofs) {
                Some(link) => Child::Link(link),
                None => Child::End,
            },
        }
    }

    fn child_mut(&mut self, _ofs: usize) -> ChildMut<Self, A> {
        todo!()
    }
}

enum InsertResult<V> {
    Ok,
    Replaced(V),
    Split,
}

enum RemoveResult<V> {
    None,
    Removed(V),
    Underflow,
}

impl<K, V, A> BTreeMap<K, V, A>
where
    K: Fundamental + Ord,
    V: WellFormed,
    V::Archived: WellArchived<V>,
    A: Fundamental + Annotation<Pair<K, V>> + Borrow<MaxKey<K>>,
{
    /// Create a new empty BTreemap
    pub fn new() -> Self {
        Self::default()
    }

    /// Insert a key-value pair into the map, returning the replaced value if
    /// any.
    pub fn insert(&mut self, k: K, v: V) -> Option<V> {
        match self.sub_insert(k, v) {
            InsertResult::Ok => None,
            InsertResult::Replaced(v) => Some(v),
            InsertResult::Split => todo!(),
        }
    }

    fn sub_insert(&mut self, k: K, v: V) -> InsertResult<V> {
        println!("sub insert");
        match &mut self.0 {
            BTreeMapInner::LeafNode(leaves) => match leaves.insert(k, v) {
                Ok(Some(op)) => InsertResult::Replaced(op),
                Ok(None) => InsertResult::Ok,
                Err(split) => {
                    let a = mem::take(leaves);
                    let b = split;
                    *self = BTreeMap(BTreeMapInner::LinkNode(
                        LinkNode::from_leaf_nodes(a, b),
                    ));
                    InsertResult::Ok
                }
            },
            BTreeMapInner::LinkNode(links) => {
                // default to last link
                let mut idx = links.0.len() - 1;

                for i in 0..links.0.len() {
                    match (*links.0[i].annotation()).borrow() {
                        MaxKey::NegativeInfinity => unreachable!(),
                        MaxKey::Maximum(key) => {
                            if let Ordering::Greater = key.cmp(&k) {
                                idx = i;
                                break;
                            }
                        }
                    }
                }

                println!("idx = {:?}", idx);

                // insert in last
                let last = &mut links.0[idx];
                let inner = last.inner_mut();
                let res = inner.sub_insert(k, v);
                if let InsertResult::Split = res {
                    todo!()
                } else {
                    res
                }
            }
        }
    }

    /// Get a reference to the value of the key `k`, if any
    fn get<O>(&mut self, k: &O) -> Option<&V>
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
    fn remove<O>(&mut self, k: &O) -> Option<V>
    where
        K: Borrow<O>,
        O: Ord,
    {
        match &mut self.0 {
            BTreeMapInner::LeafNode(leaves) => leaves.remove(k),
            BTreeMapInner::LinkNode(links) => todo!(),
        }
    }
}

#[cfg(test)]
mod test {
    use crate::MaxKey;

    use super::BTreeMap;

    use rkyv::rend::LittleEndian;

    #[test]
    fn btree_write_read() {
        let mut map =
            BTreeMap::<LittleEndian<i32>, i32, MaxKey<LittleEndian<i32>>>::new(
            );

        const N: i32 = 128;

        for o in 0..N {
            println!("\n------------\no = {}", o);

            for i in 0..o {
                println!("insert {:?}", i);
                assert_eq!(map.insert(LittleEndian::from(i), i), None);
            }

            for i in 0..o {
                println!("re-insert {:?}", i);
                assert_eq!(map.insert(LittleEndian::from(i), i + 1), Some(i));
            }

            for i in 0..o {
                assert_eq!(map.get(&LittleEndian::from(i)), Some(&(i + 1)));
            }

            for i in 0..o {
                assert_eq!(map.remove(&LittleEndian::from(i)), Some(i + 1));
            }

            // reverse

            println!("-------------\nreverse");

            for i in 0..o {
                let i = o - i - 1;
                assert_eq!(map.insert(LittleEndian::from(i), i), None);
            }

            for i in 0..o {
                let i = o - i - 1;
                assert_eq!(map.remove(&LittleEndian::from(i)), Some(i));
            }
        }
    }
}
