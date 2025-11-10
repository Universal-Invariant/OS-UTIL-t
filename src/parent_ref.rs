use std::rc::{Rc, Weak};
use std::cell::RefCell;



/// A safe, ergonomic wrapper for `Option<Weak<RefCell<T>>>` used as a parent pointer.
#[derive(Debug)]
pub struct ParentRef<T> {
    weak: Option<Weak<RefCell<T>>>,
}

impl<T> Clone for ParentRef<T> {
    fn clone(&self) -> Self {
        Self {
            weak: self.weak.clone(), // This works for any T!
        }
    }
}

impl<T> ParentRef<T> {
    /// Creates a new empty parent reference (for root nodes).
    pub fn none() -> Self {
        Self { weak: None }
    }

    /// Creates a parent reference from a strong reference.
    pub fn from_rc(rc: &Rc<RefCell<T>>) -> Self {
        Self {
            weak: Some(Rc::downgrade(rc)),
        }
    }

	pub fn is_none(&self) -> bool {
    	self.weak.is_none()
	}

	pub fn is_some(&self) -> bool {
    	self.weak.is_some()
	}

    /// Upgrades the weak reference to a strong reference if the parent is still alive.
    pub fn upgrade(&self) -> Option<Rc<RefCell<T>>> {
        self.weak.as_ref().and_then(|w| w.upgrade())
    }

    /// Checks if this parent reference points to a valid (alive) parent.
    pub fn is_valid(&self) -> bool {
        self.weak.as_ref().map_or(false, |w| w.upgrade().is_some())
    }

    pub fn with<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&T) -> R,
    {
        self.upgrade().map(|rc| f(&*rc.borrow()))
    }

    pub fn with_mut<R, F>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut T) -> R,
    {
        self.upgrade().map(|rc| f(&mut *rc.borrow_mut()))
    }


}

impl<T> Default for ParentRef<T> {
    fn default() -> Self {
        Self::none()
    }
}

// Optional: Implement PartialEq if T supports it (useful for testing)
impl<T> PartialEq for ParentRef<T> {
    fn eq(&self, other: &Self) -> bool {
        // Compare by pointer identity if both are valid
        match (self.upgrade(), other.upgrade()) {
            (Some(a), Some(b)) => Rc::ptr_eq(&a, &b),
            (None, None) => true,
            _ => false,
        }
    }
}

