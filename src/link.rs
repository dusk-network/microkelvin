use alloc::rc::Rc;
use core::cell::{RefCell, RefMut};
use core::mem;
use core::ops::{Deref, DerefMut};

use canonical::{Canon, CanonError, Id};

// when not using persistance, the PStore is just a unit struct.

use crate::{Annotation, Compound};

#[derive(Debug, Clone)]
enum LinkInner<C, A> {
    Placeholder,
    C(Rc<C>),
    Ca(Rc<C>, A),
    Ia(Id, A),
    #[allow(unused)]
    Ica(Id, Rc<C>, A),
}

impl<C, A> Default for LinkInner<C, A> {
    fn default() -> Self {
        Self::Placeholder
    }
}

#[derive(Clone)]
/// TODO
pub struct Link<C, A> {
    inner: RefCell<LinkInner<C, A>>,
}

impl<C: core::fmt::Debug, A: core::fmt::Debug> core::fmt::Debug for Link<C, A> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        write!(f, "{:?}", self.inner)
    }
}

impl<C, A> Link<C, A>
where
    C: Compound<A>,
{
    /// Create a new link
    pub fn new(compound: C) -> Self
    where
        A: Annotation<C::Leaf>,
    {
        Link {
            inner: RefCell::new(LinkInner::C(Rc::new(compound))),
        }
    }

    /// Create a new link
    pub fn new_annotated(compound: C, annotation: A) -> Self
    where
        A: Annotation<C::Leaf>,
    {
        Link {
            inner: RefCell::new(LinkInner::Ca(Rc::new(compound), annotation)),
        }
    }

    /// Create a new link
    pub fn new_persisted(id: Id, annotation: A) -> Self {
        Link {
            inner: RefCell::new(LinkInner::Ia(id, annotation)),
        }
    }

    /// Returns a reference to to the annotation stored
    pub fn annotation(&self) -> LinkAnnotation<C, A>
    where
        A: Annotation<C::Leaf>,
    {
        let mut borrow = self.inner.borrow_mut();
        let a = match *borrow {
            LinkInner::Ca(_, _)
            | LinkInner::Ica(_, _, _)
            | LinkInner::Ia(_, _) => return LinkAnnotation(borrow),
            LinkInner::C(ref c) => A::combine(c.annotations()),
            LinkInner::Placeholder => unreachable!(),
        };
        if let LinkInner::C(c) = mem::take(&mut *borrow) {
            *borrow = LinkInner::Ca(c, a)
        } else {
            unreachable!()
        }
        LinkAnnotation(borrow)
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn compound(&self) -> Result<LinkCompound<C, A>, CanonError> {
        let borrow: RefMut<LinkInner<C, A>> = self.inner.borrow_mut();
        match *borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(_) | LinkInner::Ca(_, _) | LinkInner::Ica(_, _, _) => {
                return Ok(LinkCompound(borrow))
            }
            LinkInner::Ia(_, _) => todo!(),
        }
    }

    /// Gets a reference to the inner compound of the link'
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn into_compound(self) -> Result<C, CanonError>
    where
        C: Clone,
        C::Leaf: Canon,
        A: Canon,
    {
        let inner = self.inner.into_inner();
        match inner {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(rc)
            | LinkInner::Ca(rc, _)
            | LinkInner::Ica(_, rc, _) => match Rc::try_unwrap(rc) {
                Ok(c) => Ok(c),
                Err(rc) => Ok((&*rc).clone()),
            },
            LinkInner::Ia(id, _) => C::from_generic(&id.reify()?),
        }
    }

    /// Computes the Id of the
    pub fn id(&self) -> Id
    where
        C::Leaf: Canon,
        A: Annotation<C::Leaf> + Canon,
    {
        let borrow = self.inner.borrow();
        match &*borrow {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(c) | LinkInner::Ca(c, _) => {
                let gen = c.generic();
                Id::new(&gen)
            }
            LinkInner::Ia(id, _) | LinkInner::Ica(id, _, _) => *id,
        }
    }

    /// Returns a Mutable reference to the underlying compound node
    ///
    /// Drops cached annotations and ids
    ///
    /// Can fail when trying to fetch data over i/o
    pub fn compound_mut(
        &mut self,
    ) -> Result<LinkCompoundMut<C, A>, CanonError> {
        let mut borrow: RefMut<LinkInner<C, A>> = self.inner.borrow_mut();

        match mem::take(&mut *borrow) {
            LinkInner::Placeholder => unreachable!(),
            LinkInner::C(c) | LinkInner::Ca(c, _) | LinkInner::Ica(_, c, _) => {
                *borrow = LinkInner::C(c);
                return Ok(LinkCompoundMut(borrow));
            }
            LinkInner::Ia(_, _) => {
                todo!()
            }
        }
    }
}

/// A wrapped borrow of an inner link guaranteed to contain a computed
/// annotation
#[derive(Debug)]
pub struct LinkAnnotation<'a, C, A>(RefMut<'a, LinkInner<C, A>>);

/// A wrapped borrow of an inner node guaranteed to contain a compound node
#[derive(Debug)]
pub struct LinkCompound<'a, C, A>(RefMut<'a, LinkInner<C, A>>);

/// A wrapped mutable borrow of an inner node guaranteed to contain a compound
/// node
#[derive(Debug)]
pub struct LinkCompoundMut<'a, C, A>(RefMut<'a, LinkInner<C, A>>);

impl<'a, C, A> Deref for LinkAnnotation<'a, C, A> {
    type Target = A;

    fn deref(&self) -> &Self::Target {
        match *self.0 {
            LinkInner::Ica(_, _, ref a)
            | LinkInner::Ia(_, ref a)
            | LinkInner::Ca(_, ref a) => a,
            _ => unreachable!(),
        }
    }
}

impl<'a, C, A> Deref for LinkCompound<'a, C, A> {
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match *self.0 {
            LinkInner::C(ref c)
            | LinkInner::Ca(ref c, _)
            | LinkInner::Ica(_, ref c, _) => c,
            _ => unreachable!(),
        }
    }
}

impl<'a, C, A> Deref for LinkCompoundMut<'a, C, A>
where
    C: Clone,
{
    type Target = C;

    fn deref(&self) -> &Self::Target {
        match *self.0 {
            LinkInner::C(ref c) => c,
            _ => unreachable!(),
        }
    }
}

impl<'a, C, A> DerefMut for LinkCompoundMut<'a, C, A>
where
    C: Clone,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match *self.0 {
            LinkInner::C(ref mut c) => Rc::make_mut(c),
            _ => unreachable!(),
        }
    }
}
