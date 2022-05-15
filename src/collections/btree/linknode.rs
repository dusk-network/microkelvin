use core::borrow::{Borrow, BorrowMut};
use core::cmp::Ordering;
use core::fmt::Debug;

use crate::{
    Annotation, Fundamental, Link, MaxKey, MaybeStored, StoreProvider,
    StoreSerializer, TreeViz, WellArchived, WellFormed,
};

use rkyv::ser::{ScratchSpace, Serializer};
use rkyv::{Archive, Deserialize, Serialize};

use bytecheck::CheckBytes;

use super::btreemap::{BTreeMap, BTreeMapInner, Insert, Pair, Remove};
use super::leafnode::LeafNode;

fn node_search<'a, O, K, V, A, const LE: usize, const LI: usize>(
    o: &'a O,
) -> impl Fn(&Link<BTreeMap<K, V, A, LE, LI>, A>) -> Ordering + 'a
where
    O: Ord + Debug,
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

#[derive(Archive, Clone, Serialize, Deserialize)]
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

impl<K, V, A, const LE: usize, const LI: usize> Debug
    for LinkNode<K, V, A, LE, LI>
where
    K: Fundamental + Debug,
    V: WellFormed + Debug,
    V::Archived: WellArchived<V> + Debug,
    A: Annotation<Pair<K, V>> + Fundamental + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        for link in &self.0 {
            match link.inner() {
                MaybeStored::Memory(mem) => mem.treeify(f, 0)?,
                MaybeStored::Stored(_) => todo!(),
            }
        }
        Ok(())
    }
}

fn link_search<'a, O, K, V, A, const LE: usize, const LI: usize>(
    o: &'a O,
) -> impl Fn(&Link<BTreeMap<K, V, A, LE, LI>, A>) -> Ordering + 'a
where
    O: Ord + Debug,
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

    pub(crate) fn from_link_nodes(
        a: LinkNode<K, V, A, LE, LI>,
        b: LinkNode<K, V, A, LE, LI>,
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
        O: Ord + Debug,
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
        O: Ord + Debug,
    {
        let i = match self.0.binary_search_by(link_search(o)) {
            Ok(i) => i,
            Err(i) => core::cmp::min(i, self.0.len() - 1),
        };
        println!("remove entering {:?}", i);

        let remove = match self.0[i].inner_mut() {
            BTreeMap(BTreeMapInner::LeafNode(le)) => le.remove(o),
            BTreeMap(BTreeMapInner::LinkNode(li)) => li.remove(o),
        };

        // remove completed

        match remove {
            Remove::None => Remove::None,
            Remove::Removed(v) => Remove::Removed(v),
            Remove::Underflow(v) => {
                println!("underflow in linknode \n{:?}", self);
                let removed = self.0.remove(i).into_inner();

                let links = &mut *self.0;

                let (left, right) = links.split_at_mut(i);

                let sibling_left = left.last_mut();
                let sibling_right = right.first_mut();

                match (sibling_left, sibling_right) {
                    (None, None) => Remove::Underflow(v),
                    (_, Some(right_link)) => {
                        match right_link.inner_mut().prepend(removed) {
                            Ok(()) => Remove::Removed(v),
                            Err(rest) => {
                                todo!()
                            }
                        }
                    }
                    (Some(prev), None) => {
                        todo!()
                    }
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
                        println!("splutt");
                        let link =
                            Link::new(BTreeMap(BTreeMapInner::LeafNode(ln)));

                        if !self.full() {
                            self.0.push(link);
                            Insert::Ok
                        } else {
                            println!("split?");
                            let mut split = self.split();
                            println!("split {:?}", split);
                            split.append_link(link);
                            Insert::Split(split)
                        }
                    }
                }
            }
            Some(BTreeMap(BTreeMapInner::LinkNode(li))) => {
                match li.insert_leaf(k, v) {
                    Insert::Ok => Insert::Ok,
                    Insert::Replaced(v) => Insert::Replaced(v),
                    Insert::Split(li) => {
                        println!("splutt");
                        let link =
                            Link::new(BTreeMap(BTreeMapInner::LinkNode(li)));

                        if !self.full() {
                            self.0.push(link);
                            Insert::Ok
                        } else {
                            println!("split?");
                            let mut split = self.split();
                            println!("split {:?}", split);
                            split.append_link(link);
                            Insert::Split(split)
                        }
                    }
                }
            }
            None => todo!(),
        }
    }

    fn split(&mut self) -> Self {
        LinkNode(self.0.split_off((LI + 1) / 2))
    }

    pub(crate) fn append_link(
        &mut self,
        link: Link<BTreeMap<K, V, A, LE, LI>, A>,
    ) {
        self.0.push(link)
    }

    fn split_off(&mut self, at: usize) -> Self {
        LinkNode(self.0.split_off(at))
    }

    pub(crate) fn prepend(&mut self, other: &mut Self) -> Result<(), ()> {
        let cap = self.remaining_capacity();
        let needed = other.len();

        // example

        // self [2, 3, 4] prepended with [0, 1].

        if cap >= needed {
            other.0.append(&mut self.0);
            *self = other;
            Ok(())
        } else {
            // make room by splitting.

            println!("gorka");

            let total_len = self.len() + other.len();

            let ideal_len = total_len / 2;

            let split_at = ideal_len - other.len();

            let last = self.split_off(split_at);

            debug_assert!(self.prepend(other).is_none());

            println!("returning {:?}", last);

            Some(last)
        }
    }
}
