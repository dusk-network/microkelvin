use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::Debug;

use crate::collections::btree::btreemap::BTreeMapInner;
use crate::{
    Annotation, Fundamental, Link, MaxKey, MaybeStored, StoreProvider,
    StoreSerializer, WellArchived, WellFormed,
};

use rkyv::ser::{ScratchSpace, Serializer};
use rkyv::{Archive, Deserialize, Serialize};

use bytecheck::CheckBytes;

use super::btreemap::{BTreeMap, Insert, Pair, Remove};
use super::leafnode::LeafNode;

fn node_search<'a, O, K, V, A, const LE: usize, const LI: usize>(
    o: &'a O,
) -> impl Fn(&Link<BTreeMap<K, V, A, LE, LI>, A>) -> Ordering + 'a
where
    O: Ord,
    K: 'a + Ord + Fundamental + Borrow<O> + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Fundamental + Annotation<Pair<K, V>> + Borrow<MaxKey<K>> + Debug,
{
    move |link: &Link<BTreeMap<K, V, A, LE, LI>, A>| {
        let ann = &*link.annotation();
        let max: &MaxKey<K> = ann.borrow();
        max.partial_cmp(o).expect("Always ordered")
    }
}

#[derive(Archive, Clone, Serialize, Deserialize, Debug)]
#[archive_attr(derive(CheckBytes))]
#[archive(bound(serialize = "
  K: Fundamental + Debug,
  V: WellFormed + Debug,
  V::Archived: WellArchived<V> + Debug,
  A: Fundamental + Annotation<Pair<K, V>> + Debug,
  __S: Sized + Serializer + BorrowMut<StoreSerializer> + ScratchSpace"))]
#[archive(bound(deserialize = "
  A: Fundamental,
  __D: StoreProvider,"))]
/// TODO make private.
pub struct LinkNode<K, V, A, const LE: usize, const LI: usize>(
    #[omit_bounds] Vec<Link<BTreeMap<K, V, A, LE, LI>, A>>,
);

impl<K, V, A, const LE: usize, const LI: usize> Default
    for LinkNode<K, V, A, LE, LI>
{
    fn default() -> Self {
        Self(Default::default())
    }
}

fn link_search<'a, O, K, V, A, const LE: usize, const LI: usize>(
    o: &'a O,
) -> impl Fn(&Link<BTreeMap<K, V, A, LE, LI>, A>) -> Ordering + 'a
where
    O: Ord,
    K: 'a + Ord + Fundamental + Borrow<O> + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Fundamental + Annotation<Pair<K, V>> + Borrow<MaxKey<K>> + Debug,
{
    move |link: &Link<BTreeMap<K, V, A, LE, LI>, A>| {
        let ann = &*link.annotation();
        let max: &MaxKey<K> = ann.borrow();
        max.partial_cmp(o).expect("Always ordered")
    }
}

pub enum Append<T> {
    Ok,
    Split(T),
}

impl<K, V, A, const LE: usize, const LI: usize> LinkNode<K, V, A, LE, LI>
where
    K: Fundamental + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Fundamental + Annotation<Pair<K, V>> + Debug,
    A::Archived: Debug,
{
    #[inline(always)]
    fn underflow(&self) -> bool {
        self.len() <= LI / 2
    }

    #[inline(always)]
    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    #[inline(always)]
    fn full(&self) -> bool {
        self.remaining_capacity() == 0
    }

    #[inline(always)]
    fn remaining_capacity(&self) -> usize {
        LI - self.len()
    }

    pub(crate) fn from_leaf_nodes(
        a: LeafNode<K, V, LE>,
        b: LeafNode<K, V, LE>,
    ) -> Self {
        let map_a = BTreeMap::from(a);
        let map_b = BTreeMap::from(b);
        let link_a = Link::new(map_a);
        let link_b = Link::new(map_b);
        LinkNode(vec![link_a, link_b])
    }

    pub(crate) fn get_link(
        &self,
        ofs: usize,
    ) -> Option<&Link<BTreeMap<K, V, A, LE, LI>, A>> {
        self.0.get(ofs)
    }

    pub(crate) fn remove_link(
        &mut self,
        ofs: usize,
    ) -> Link<BTreeMap<K, V, A, LE, LI>, A> {
        self.0.remove(ofs)
    }

    pub(crate) fn get<O>(&self, o: &O) -> Option<&V>
    where
        K: Ord + Borrow<O>,
        A: Borrow<MaxKey<K>>,
        O: Ord,
    {
        match self.0.binary_search_by(link_search(o)) {
            Ok(i) | Err(i) => match self.0[i].inner() {
                MaybeStored::Memory(map) => map.get(o),
                MaybeStored::Stored(_) => todo!(),
            },
        }
    }

    pub(crate) fn remove<O>(&mut self, o: &O) -> Remove<V>
    where
        K: Ord + Borrow<O>,
        A: Borrow<MaxKey<K>>,
        O: Ord,
    {
        if self.len() == 0 {
            return Remove::None;
        };

        let i = match self.0.binary_search_by(link_search(o)) {
            Ok(i) => i,
            Err(i) => core::cmp::min(i, self.0.len() - 1),
        };
        println!("remove entering {:?}", i);

        let inner = self.0[i].inner_mut();

        match inner.sub_remove(o) {
            rem @ Remove::None | rem @ Remove::Removed(_) => rem,
            Remove::Underflow(v) => {
                let taken = self.0.remove(i).into_inner();

                // same index is now the next node, wich may or may not exist
                match (taken, self.0.get_mut(i).map(Link::inner_mut)) {
                    (
                        BTreeMap(BTreeMapInner::LeafNode(removed_le)),
                        Some(BTreeMap(BTreeMapInner::LeafNode(self_le))),
                    ) => {
                        self_le.prepend(removed_le);
                    }
                    _ => todo!(),
                }

                // if let Some(next) = self.0.get_mut(i) {

                //     match next.inner_mut() {
                //     }

                //     if let Some(leaves) = next.prepend(taken) {
                //         self.0.insert(i + 1, Link::new(leaves))
                //     } else {
                //         ()
                //     }
                // } else {
                //     if i > 0 {
                //         let prev = self.0[i - 1].inner_mut();
                //         println!("appending here?");

                //         if taken.len() > 0 {
                //             prev.append(taken);
                //         }
                //     }
                // }

                if self.underflow() {
                    println!("underflow in link?");

                    Remove::Underflow(v)
                } else {
                    Remove::Removed(v)
                }
            }
        }
    }

    pub(crate) fn insert_leaf(&mut self, k: K, v: V) -> Insert<V, Self>
    where
        K: Ord,
        A: Borrow<MaxKey<K>>,
    {
        println!("insert leaf in linknode");
        dbg!(&self);

        let i = match self.0.binary_search_by(link_search(&k)) {
            Ok(i) => i,
            Err(i) => core::cmp::min(i, self.0.len() - 1),
        };

        match self.0.get_mut(i).map(Link::inner_mut) {
            Some(BTreeMap(BTreeMapInner::LeafNode(le))) => {
                match le.insert_leaf(k, v) {
                    Insert::Ok => Insert::Ok,
                    Insert::Replaced(v) => Insert::Replaced(v),
                    Insert::Split(ln) => {
                        todo!()
                    }
                }
            }
            Some(BTreeMap(BTreeMapInner::LinkNode(li))) => todo!(),
            None => todo!(),
        }
    }

    fn prepend(&mut self, mut other: Self) -> Option<Self> {
        let cap = self.remaining_capacity();
        let needed = other.len();

        // example

        // self [2, 3, 4] prepended with [0, 1].

        if cap >= needed {
            other.0.append(&mut self.0);
            *self = other;
        }

        None
    }

    pub(crate) fn append_link(
        &mut self,
        _link: Link<BTreeMap<K, V, A, LE, LI>, A>,
    ) -> Append<Self> {
        todo!()
    }

    fn append(&mut self, mut other: Self) -> Option<Self> {
        let cap = self.remaining_capacity();
        let needed = other.len();

        // example

        // self [2, 3, 4] prepended with [0, 1].

        if cap >= needed {
            self.0.append(&mut other.0);
        }

        None
    }
}
