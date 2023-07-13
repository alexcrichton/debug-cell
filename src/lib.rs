// Copyright 2012-2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! A clone of the standard library's `RefCell` type with extra debugging
//! support in non-release builds.
//!
//! Whenever a borrow error happens the current
//! locations of where known borrows were created will be printed out as well.
//!
//! # Examples
//!
//! ```no_run
//! use debug_cell::RefCell;
//!
//! let r = RefCell::new(3);
//! let a = r.borrow();
//!
//! // In debug builds this will print that the cell is currently borrowed
//! // above, and in release builds it will behave the same as the standard
//! // library's `RefCell`
//! let b = r.borrow_mut();
//! ```
#![deny(missing_docs)]

/// Error kind ported from nightly std
pub mod error {
    #[cfg(debug_assertions)]
    fn locations_display(locations: &[super::Location]) -> String {
        locations
            .iter()
            .map(|location| format!("  ---------------\n  {location}"))
            .collect::<Vec<_>>()
            .join("\n")
    }
    /// An error returned by [`RefCell::try_borrow`].
    #[non_exhaustive]
    #[derive(Debug)]
    pub struct BorrowError {
        /// Debug-only location of attempted borrow
        #[cfg(debug_assertions)]
        pub attempted_at: super::Location,
        /// Debug-only location of all current locations
        #[cfg(debug_assertions)]
        pub already_borrowed_at: Vec<super::Location>,
    }

    impl std::error::Error for BorrowError {}

    impl std::fmt::Display for BorrowError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            #[cfg(debug_assertions)]
            {
                write!(
                    f,
                    "Value is already borrowed mutably, current active borrows: \n{}\n\n",
                    locations_display(&self.already_borrowed_at)
                )
            }
            #[cfg(not(debug_assertions))]
            {
                write!(f, "Value is already borrowed mutably")
            }
        }
    }

    impl std::error::Error for BorrowMutError {}

    /// An error returned by [`RefCell::try_borrow_mut`].
    #[derive(Debug)]
    #[non_exhaustive]
    pub struct BorrowMutError {
        /// Debug-only location of attempted borrow
        #[cfg(debug_assertions)]
        pub attempted_at: super::Location,
        /// Debug-only locations of all current borrows
        #[cfg(debug_assertions)]
        pub already_borrowed_at: Vec<super::Location>,
    }

    impl std::fmt::Display for BorrowMutError {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            #[cfg(debug_assertions)]
            {
                write!(
                    f,
                    "Value is already borrowed, current active borrows:\n{}\n\n",
                    locations_display(&self.already_borrowed_at)
                )
            }
            #[cfg(not(debug_assertions))]
            {
                write!(f, "Value is already borrowed")
            }
        }
    }
}

#[cfg(debug_assertions)]
use std::cell::RefCell as StdRefCell;
use std::cell::{Cell, UnsafeCell};
use std::ops::{Deref, DerefMut};

/// A clone of the standard library's `RefCell` type.
pub struct RefCell<T: ?Sized> {
    borrow: BorrowFlag,
    value: UnsafeCell<T>,
}

#[cfg(not(debug_assertions))]
type Location = ();

#[cfg(debug_assertions)]
type Location = &'static std::panic::Location<'static>;

/// An enumeration of values returned from the `state` method on a `RefCell<T>`.
#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BorrowState {
    /// The cell is currently being read, there is at least one active `borrow`.
    Reading,
    /// The cell is currently being written to, there is an active `borrow_mut`.
    Writing,
    /// There are no outstanding borrows on this cell.
    Unused,
}

// Values [1, MAX-1] represent the number of `Ref` active
// (will not outgrow its range since `usize` is the size of the address space)
struct BorrowFlag {
    flag: Cell<usize>,

    #[cfg(debug_assertions)]
    locations: StdRefCell<Vec<Location>>,
}

const UNUSED: usize = 0;
const WRITING: usize = !0;

impl<T> RefCell<T> {
    /// Creates a new `RefCell` containing `value`.
    pub fn new(value: T) -> RefCell<T> {
        RefCell {
            borrow: BorrowFlag::new(),
            value: UnsafeCell::new(value),
        }
    }

    /// Consumes the `RefCell`, returning the wrapped value.
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn into_inner(self) -> T {
        debug_assert!(self.borrow.flag.get() == UNUSED);
        self.value.into_inner()
    }
}

impl<T: ?Sized> RefCell<T> {
    /// Immutably borrows the wrapped value.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple
    /// immutable borrows can be taken out at the same time.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently mutably borrowed.
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn borrow(&self) -> Ref<'_, T> {
        match self.try_borrow() {
            Ok(value) => value,
            Err(message) => panic!(
                "Borrowing {} immutably failed: {}",
                std::any::type_name::<Self>(),
                message
            ),
        }
    }
    /// Immutably borrows the wrapped value.
    ///
    /// The borrow lasts until the returned `Ref` exits scope. Multiple
    /// immutable borrows can be taken out at the same time.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently mutably borrowed.
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn try_borrow(&self) -> Result<Ref<'_, T>, crate::error::BorrowError> {
        match BorrowRef::new(&self.borrow) {
            Some(b) => Ok(Ref {
                _value: unsafe { &*self.value.get() },
                _borrow: b,
            }),
            None => {
                #[cfg(debug_assertions)]
                {
                    Err(crate::error::BorrowError {
                        attempted_at: get_caller(),
                        already_borrowed_at: self.borrow.locations.borrow().clone(),
                    })
                }
                #[cfg(not(debug_assertions))]
                {
                    Err(crate::error::BorrowError {})
                }
            }
        }
    }

    /// Mutably borrows the wrapped value.
    ///
    /// The borrow lasts until the returned `RefMut` exits scope. The value
    /// cannot be borrowed while this borrow is active.
    ///
    /// # Panics
    ///
    /// Panics if the value is currently borrowed.
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn borrow_mut(&self) -> RefMut<'_, T> {
        match self.try_borrow_mut() {
            Ok(value) => value,
            Err(message) => panic!(
                "Borrowing {} mutably failed: {}",
                std::any::type_name::<Self>(),
                message
            ),
        }
    }

    /// Tries borrowing the wrapped value mutably.
    ///
    /// The borrow lasts until the returned `RefMut` exits scope. The value
    /// cannot be borrowed while this borrow is active.
    ///
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(debug_assertions, track_caller)]
    pub fn try_borrow_mut(&self) -> Result<RefMut<'_, T>, error::BorrowMutError> {
        match BorrowRefMut::new(&self.borrow) {
            Some(b) => Ok(RefMut {
                _value: unsafe { &mut *self.value.get() },
                _borrow: b,
            }),
            None => {
                #[cfg(debug_assertions)]
                {
                    Err(error::BorrowMutError {
                        attempted_at: get_caller(),
                        already_borrowed_at: self.borrow.locations.borrow().clone(),
                    })
                }
                #[cfg(not(debug_assertions))]
                {
                    Err(error::BorrowMutError {})
                }
            }
        }
    }
}

#[cfg(not(debug_assertions))]
impl BorrowFlag {
    #[inline]
    fn new() -> BorrowFlag {
        BorrowFlag {
            flag: Cell::new(UNUSED),
        }
    }

    #[inline]
    fn push(&self, _caller: Location) {}

    #[inline]
    fn pop(&self) {}
}

#[cfg(debug_assertions)]
impl BorrowFlag {
    fn new() -> BorrowFlag {
        BorrowFlag {
            flag: Cell::new(UNUSED),
            locations: StdRefCell::new(Vec::new()),
        }
    }

    fn push(&self, caller: Location) {
        self.locations.borrow_mut().push(caller);
    }

    fn pop(&self) {
        self.locations.borrow_mut().pop();
    }
}

#[cfg(not(debug_assertions))]
#[inline]
fn get_caller() -> Location {}

#[cfg(debug_assertions)]
#[inline(never)]
#[track_caller]
fn get_caller() -> Location {
    std::panic::Location::caller()
}

unsafe impl<T: ?Sized> Send for RefCell<T> where T: Send {}

impl<T: Clone> Clone for RefCell<T> {
    #[inline]
    fn clone(&self) -> RefCell<T> {
        RefCell::new(self.borrow().clone())
    }
}

impl<T: Default> Default for RefCell<T> {
    #[inline]
    fn default() -> RefCell<T> {
        RefCell::new(Default::default())
    }
}

impl<T: ?Sized + PartialEq> PartialEq for RefCell<T> {
    #[inline]
    fn eq(&self, other: &RefCell<T>) -> bool {
        *self.borrow() == *other.borrow()
    }
}

impl<T: ?Sized + Eq> Eq for RefCell<T> {}

struct BorrowRef<'b> {
    borrow: &'b BorrowFlag,
}

impl<'b> BorrowRef<'b> {
    #[cfg_attr(not(debug_assertions), inline)]
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(debug_assertions, track_caller)]
    fn new(borrow: &'b BorrowFlag) -> Option<BorrowRef<'b>> {
        let flag = borrow.flag.get();
        if flag == WRITING {
            return None;
        }
        borrow.flag.set(flag + 1);
        borrow.push(get_caller());
        Some(BorrowRef { borrow })
    }
}

impl<'b> Drop for BorrowRef<'b> {
    #[inline]
    fn drop(&mut self) {
        let flag = self.borrow.flag.get();
        debug_assert!(flag != WRITING && flag != UNUSED);
        self.borrow.flag.set(flag - 1);
        self.borrow.pop();
    }
}

/// Wraps a borrowed reference to a value in a `RefCell` box.
/// A wrapper type for an immutably borrowed value from a `RefCell<T>`.
///
/// See the [module-level documentation](index.html) for more.

pub struct Ref<'b, T: ?Sized + 'b> {
    // FIXME #12808: strange name to try to avoid interfering with
    // field accesses of the contained type via Deref
    _value: &'b T,
    _borrow: BorrowRef<'b>,
}

impl<'b, T: ?Sized + 'b> Ref<'b, T> {
    /// Makes a new `Ref` for a component of the borrowed data.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as `Ref::map(...)`.
    /// A method would interfere with methods of the same name on the contents
    /// of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// let b1: Ref<'_, (u32, char)> = c.borrow();
    /// let b2: Ref<'_, u32> = Ref::map(b1, |t| &t.0);
    /// assert_eq!(*b2, 5)
    /// ```
    #[inline]
    pub fn map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Ref<'b, U>
    where
        F: FnOnce(&T) -> &U,
    {
        Ref {
            _value: f(&*orig._value),
            _borrow: orig._borrow,
        }
    }
}

impl<'b, T: ?Sized> Deref for Ref<'b, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self._value
    }
}

struct BorrowRefMut<'b> {
    borrow: &'b BorrowFlag,
}

impl<'b> BorrowRefMut<'b> {
    #[cfg_attr(not(debug_assertions), inline)]
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(debug_assertions, track_caller)]
    fn new(borrow: &'b BorrowFlag) -> Option<BorrowRefMut<'b>> {
        if borrow.flag.get() != UNUSED {
            return None;
        }
        borrow.flag.set(WRITING);
        borrow.push(get_caller());
        Some(BorrowRefMut { borrow })
    }
}

impl<'b> Drop for BorrowRefMut<'b> {
    #[inline]
    fn drop(&mut self) {
        debug_assert!(self.borrow.flag.get() == WRITING);
        self.borrow.flag.set(UNUSED);
        self.borrow.pop();
    }
}

/// A wrapper type for a mutably borrowed value from a `RefCell<T>`.
pub struct RefMut<'b, T: ?Sized + 'b> {
    // FIXME #12808: strange name to try to avoid interfering with
    // field accesses of the contained type via Deref
    _value: &'b mut T,
    _borrow: BorrowRefMut<'b>,
}

impl<'b, T: ?Sized + 'b> RefMut<'b, T> {
    /// Makes a new `RefMut` for a component of the borrowed data, e.g., an enum
    /// variant.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::map(...)`. A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// {
    ///     let b1: RefMut<'_, (u32, char)> = c.borrow_mut();
    ///     let mut b2: RefMut<'_, u32> = RefMut::map(b1, |t| &mut t.0);
    ///     assert_eq!(*b2, 5);
    ///     *b2 = 42;
    /// }
    /// assert_eq!(*c.borrow(), (42, 'b'));
    /// ```
    pub fn map<U: ?Sized, F>(orig: RefMut<'b, T>, f: F) -> RefMut<'b, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        RefMut {
            _value: f(&mut *orig._value),
            _borrow: orig._borrow,
        }
    }
}

impl<'b, T: ?Sized> Deref for RefMut<'b, T> {
    type Target = T;
    fn deref(&self) -> &T {
        self._value
    }
}

impl<'b, T: ?Sized> DerefMut for RefMut<'b, T> {
    fn deref_mut(&mut self) -> &mut T {
        self._value
    }
}

#[cfg(test)]
mod tests {
    use super::RefCell;

    #[test]
    fn ok_borrows() {
        let a = RefCell::new(2);
        let b = a.borrow();
        let c = a.borrow();
        assert_eq!(*b, 2);
        assert_eq!(*c, 2);
        drop((b, c));

        let mut b = a.borrow_mut();
        assert_eq!(*b, 2);
        *b = 4;
        drop(b);

        assert_eq!(*a.borrow(), 4);
    }

    #[test]
    fn ok_borrows_map() {
        let a = RefCell::new((0, 1));
        let b = a.borrow();
        let c = super::Ref::map(b, |v| &v.0);
        assert_eq!(*c, 0);
        assert!(a.try_borrow().is_ok());
        assert!(a.try_borrow_mut().is_err());
        drop(c);
        assert!(a.try_borrow().is_ok());
        assert!(a.try_borrow_mut().is_ok());

        let mut b = a.borrow_mut();
        assert_eq!(b.0, 0);
        b.0 = 999;
        drop(b);

        assert_eq!(a.borrow().0, 999);
    }

    #[test]
    fn ok_borrow_mut_map() {
        let a = RefCell::new((0, 1));
        let b = a.borrow_mut();
        let mut c = super::RefMut::map(b, |v| &mut v.0);
        assert_eq!(*c, 0);
        *c = 999;
        drop(c);
        assert_eq!(a.try_borrow().unwrap().0, 999);
    }

    #[should_panic]
    #[test]
    fn bad_borrow_mut() {
        let a = RefCell::new(2);
        let _a = a.borrow();
        a.borrow_mut();
    }

    #[should_panic]
    #[test]
    fn bad_borrow() {
        let a = RefCell::new(2);
        let _a = a.borrow_mut();
        a.borrow();
    }
}
