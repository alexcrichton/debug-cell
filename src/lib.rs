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

#[cfg(debug_assertions)]
extern crate backtrace;

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
#[derive(Debug)]
struct Location {
    file: Option<String>,
    name: Option<String>,
    line: Option<u32>,
}

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
    pub fn into_inner(self) -> T {
        debug_assert!(self.borrow.flag.get() == UNUSED);
        unsafe { self.value.into_inner() }
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
    pub fn borrow<'a>(&'a self) -> Ref<'a, T> {
        match BorrowRef::new(&self.borrow) {
            Some(b) => Ref {
                _value: unsafe { &*self.value.get() },
                _borrow: b,
            },
            None => self.panic("mutably borrowed"),
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
    pub fn borrow_mut<'a>(&'a self) -> RefMut<'a, T> {
        match BorrowRefMut::new(&self.borrow) {
            Some(b) => RefMut {
                _value: unsafe { &mut *self.value.get() },
                _borrow: b,
            },
            None => self.panic("borrowed"),
        }
    }

    #[cfg(not(debug_assertions))]
    fn panic(&self, msg: &str) -> ! {
        panic!("RefCell<T> already {}", msg)
    }

    #[cfg(debug_assertions)]
    fn panic(&self, msg: &str) -> ! {
        let mut msg = format!("RefCell<T> already {}", msg);
        let locations = self.borrow.locations.borrow();
        if locations.len() > 0 {
            msg.push_str("\ncurrent active borrows: \n");
            for (i, b) in locations.iter().enumerate() {
                msg.push_str(&format!("  {} - {}:{} {}\n",
                                      i,
                                      b.file.as_ref().unwrap(),
                                      b.line.as_ref().unwrap(),
                                      b.name.as_ref().unwrap()));
            }
            msg.push_str("\n\n");
        }
        panic!(msg)
    }
}

#[cfg(not(debug_assertions))]
impl BorrowFlag {
    #[inline]
    fn new() -> BorrowFlag {
        BorrowFlag { flag: Cell::new(UNUSED) }
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

#[inline(never)]
#[cfg(debug_assertions)]
fn get_caller() -> Location {
    let mut thing = Location {
        file: None,
        line: None,
        name: None,
    };
    let mut i = 0;
    backtrace::trace(&mut |frame| {
        // Skip the first 3 frames as it's:
        //
        //  * get_caller()
        //  * BorrowRef{,Mut}::new
        //  * RefCell::borrow{,_mut}
        if i == 4 {
            let ip = frame.ip();
            backtrace::resolve(ip, &mut |symbol| {
                thing.name = symbol.name().map(|s| {
                    let s = String::from_utf8_lossy(s);
                    let mut sym = String::new();
                    let _ = backtrace::demangle(&mut sym, &s);
                    sym
                });
                thing.file = symbol.filename().map(|s| {
                    String::from_utf8_lossy(s).into_owned()
                });
                thing.line = symbol.lineno();
            });
            false // stop the backtrace
        } else {
            i += 1;
            true
        }
    });
    return thing
}

unsafe impl<T: ?Sized> Send for RefCell<T> where T: Send {}

impl<T: Clone> Clone for RefCell<T> {
    #[inline]
    fn clone(&self) -> RefCell<T> {
        RefCell::new(self.borrow().clone())
    }
}

impl<T:Default> Default for RefCell<T> {
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
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(not(debug_assertions), inline)]
    fn new(borrow: &'b BorrowFlag) -> Option<BorrowRef<'b>> {
        let flag = borrow.flag.get();
        if flag == WRITING { return None }
        borrow.flag.set(flag + 1);
        borrow.push(get_caller());
        Some(BorrowRef { borrow: borrow })
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
    #[cfg_attr(debug_assertions, inline(never))]
    #[cfg_attr(not(debug_assertions), inline)]
    fn new(borrow: &'b BorrowFlag) -> Option<BorrowRefMut<'b>> {
        if borrow.flag.get() != UNUSED { return None }
        borrow.flag.set(WRITING);
        borrow.push(get_caller());
        Some(BorrowRefMut { borrow: borrow })
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
