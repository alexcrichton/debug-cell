extern crate backtrace;

use std::cell::UnsafeCell;
use std::cell::RefCell as StdRefCell;
use std::ops::{Deref, DerefMut};

pub struct RefCell<T: ?Sized> {
    borrow: StdRefCell<BorrowFlag>,
    value: UnsafeCell<T>,
}

#[derive(Debug)]
struct Thing {
    file: Option<String>,
    name: Option<String>,
    line: Option<u32>,
}


#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum BorrowState {
    Reading,
    Writing,
    Unused,
}

// Values [1, MAX-1] represent the number of `Ref` active
// (will not outgrow its range since `usize` is the size of the address space)
struct BorrowFlag {
    flag: usize,
    things: Vec<Thing>,
}

const UNUSED: usize = 0;
const WRITING: usize = !0;

impl<T> RefCell<T> {
    pub fn new(value: T) -> RefCell<T> {
        RefCell {
            value: UnsafeCell::new(value),
            borrow: StdRefCell::new(BorrowFlag {
                flag: UNUSED,
                things: Vec::new()
            }),
        }
    }
}

impl<T: ?Sized> RefCell<T> {
    #[inline(never)]
    pub fn borrow<'a>(&'a self) -> Ref<'a, T> {
        match BorrowRef::new(&self.borrow) {
            Some(b) => {
                Ref {
                    _value: unsafe { &*self.value.get() },
                    _borrow: b,
                }
            }
            None => self.panic("mutably borrowed"),
        }
    }

    #[inline(never)]
    pub fn borrow_mut<'a>(&'a self) -> RefMut<'a, T> {
        match BorrowRefMut::new(&self.borrow) {
            Some(b) => {
                RefMut {
                    _value: unsafe { &mut *self.value.get() },
                    _borrow: b,
                }
            }
            None => self.panic("borrowed"),
        }
    }

    fn panic(&self, msg: &str) -> ! {
        let mut msg = format!("RefCell<T> already {}", msg);
        let b = self.borrow.borrow();
        if b.things.len() > 0 {
            msg.push_str("\ncurrent active borrows: \n");
            for (i, b) in b.things.iter().enumerate() {
                msg.push_str(&format!("  {} - {}:{} {}\n",
                                      i, b.file.as_ref().unwrap(),
                                      b.line.as_ref().unwrap(),
                                      b.name.as_ref().unwrap()));
            }
            msg.push_str("\n\n");
        }
        panic!(msg)
    }
}

#[cfg(not(debug_assertions))]
fn get_caller() -> Option<Thing> {
    None
}

#[inline(never)]
#[cfg(debug_assertions)]
fn get_caller() -> Option<Thing> {
    let mut thing = Thing {
        file: None,
        line: None,
        name: None,
    };
    let mut i = 0;
    backtrace::trace(&mut |frame| {
        if i == 4 {
            let ip = frame.ip();
            backtrace::resolve(ip, &mut |symbol| {
                thing.name = symbol.name().map(|s| {
                    let mut r = String::new();
                    let _ = backtrace::demangle(&mut r,
                                                std::str::from_utf8(s).unwrap());
                    r
                });
                thing.file = symbol.filename().map(|s| std::str::from_utf8(s).unwrap().to_string());
                thing.line = symbol.lineno();
            });
            false
        } else {
            i += 1;
            true
        }
    });
    Some(thing)
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
    _borrow: &'b StdRefCell<BorrowFlag>,
}

impl<'b> BorrowRef<'b> {
    #[inline(never)]
    fn new(borrow: &'b StdRefCell<BorrowFlag>) -> Option<BorrowRef<'b>> {
        {
            let mut b = borrow.borrow_mut();
            if b.flag == WRITING { return None }
            b.flag += 1;
            if let Some(t) = get_caller() {
                b.things.push(t);
            }
        }
        Some(BorrowRef { _borrow: borrow })
    }
}

impl<'b> Drop for BorrowRef<'b> {
    #[inline]
    fn drop(&mut self) {
        let mut borrow = self._borrow.borrow_mut();
        debug_assert!(borrow.flag != WRITING && borrow.flag != UNUSED);
        borrow.flag -= 1;
        borrow.things.pop();
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

    #[inline]
    fn deref<'a>(&'a self) -> &'a T {
        self._value
    }
}

impl<'b, T: ?Sized> Ref<'b, T> {

    /// Make a new `Ref` for a component of the borrowed data.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as `Ref::map(...)`.
    /// A method would interfere with methods of the same name on the contents
    /// of a `RefCell` used through `Deref`.
    ///
    /// # Example
    ///
    /// ```
    /// #![feature(cell_extras)]
    ///
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// let b1: Ref<(u32, char)> = c.borrow();
    /// let b2: Ref<u32> = Ref::map(b1, |t| &t.0);
    /// assert_eq!(*b2, 5)
    /// ```

    #[inline]
    pub fn map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Ref<'b, U>
        where F: FnOnce(&T) -> &U
    {
        Ref {
            _value: f(orig._value),
            _borrow: orig._borrow,
        }
    }

    /// Make a new `Ref` for a optional component of the borrowed data, e.g. an
    /// enum variant.
    ///
    /// The `RefCell` is already immutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `Ref::filter_map(...)`.  A method would interfere with methods of the
    /// same name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(cell_extras)]
    /// use std::cell::{RefCell, Ref};
    ///
    /// let c = RefCell::new(Ok(5));
    /// let b1: Ref<Result<u32, ()>> = c.borrow();
    /// let b2: Ref<u32> = Ref::filter_map(b1, |o| o.as_ref().ok()).unwrap();
    /// assert_eq!(*b2, 5)
    /// ```

    #[inline]
    pub fn filter_map<U: ?Sized, F>(orig: Ref<'b, T>, f: F) -> Option<Ref<'b, U>>
        where F: FnOnce(&T) -> Option<&U>
    {
        f(orig._value).map(move |new| Ref {
            _value: new,
            _borrow: orig._borrow,
        })
    }
}

impl<'b, T: ?Sized> RefMut<'b, T> {
    /// Make a new `RefMut` for a component of the borrowed data, e.g. an enum
    /// variant.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::map(...)`.  A method would interfere with methods of the same
    /// name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(cell_extras)]
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new((5, 'b'));
    /// {
    ///     let b1: RefMut<(u32, char)> = c.borrow_mut();
    ///     let mut b2: RefMut<u32> = RefMut::map(b1, |t| &mut t.0);
    ///     assert_eq!(*b2, 5);
    ///     *b2 = 42;
    /// }
    /// assert_eq!(*c.borrow(), (42, 'b'));
    /// ```

    #[inline]
    pub fn map<U: ?Sized, F>(orig: RefMut<'b, T>, f: F) -> RefMut<'b, U>
        where F: FnOnce(&mut T) -> &mut U
    {
        RefMut {
            _value: f(orig._value),
            _borrow: orig._borrow,
        }
    }

    /// Make a new `RefMut` for a optional component of the borrowed data, e.g.
    /// an enum variant.
    ///
    /// The `RefCell` is already mutably borrowed, so this cannot fail.
    ///
    /// This is an associated function that needs to be used as
    /// `RefMut::filter_map(...)`.  A method would interfere with methods of the
    /// same name on the contents of a `RefCell` used through `Deref`.
    ///
    /// # Example
    ///
    /// ```
    /// # #![feature(cell_extras)]
    /// use std::cell::{RefCell, RefMut};
    ///
    /// let c = RefCell::new(Ok(5));
    /// {
    ///     let b1: RefMut<Result<u32, ()>> = c.borrow_mut();
    ///     let mut b2: RefMut<u32> = RefMut::filter_map(b1, |o| {
    ///         o.as_mut().ok()
    ///     }).unwrap();
    ///     assert_eq!(*b2, 5);
    ///     *b2 = 42;
    /// }
    /// assert_eq!(*c.borrow(), Ok(42));
    /// ```

    #[inline]
    pub fn filter_map<U: ?Sized, F>(orig: RefMut<'b, T>, f: F) -> Option<RefMut<'b, U>>
        where F: FnOnce(&mut T) -> Option<&mut U>
    {
        let RefMut { _value, _borrow } = orig;
        f(_value).map(move |new| RefMut {
            _value: new,
            _borrow: _borrow,
        })
    }
}

struct BorrowRefMut<'b> {
    _borrow: &'b StdRefCell<BorrowFlag>,
}

impl<'b> Drop for BorrowRefMut<'b> {
    #[inline]
    fn drop(&mut self) {
        let mut borrow = self._borrow.borrow_mut();
        debug_assert!(borrow.flag == WRITING);
        borrow.flag = UNUSED;
        borrow.things.pop();
    }
}

impl<'b> BorrowRefMut<'b> {
    #[inline(never)]
    fn new(borrow: &'b StdRefCell<BorrowFlag>) -> Option<BorrowRefMut<'b>> {
        {
            let mut b = borrow.borrow_mut();
            if b.flag != UNUSED { return None }
            b.flag = WRITING;
            if let Some(t) = get_caller() {
                b.things.push(t);
            }
        }
        Some(BorrowRefMut { _borrow: borrow })
    }
}

/// A wrapper type for a mutably borrowed value from a `RefCell<T>`.
///
/// See the [module-level documentation](index.html) for more.

pub struct RefMut<'b, T: ?Sized + 'b> {
    // FIXME #12808: strange name to try to avoid interfering with
    // field accesses of the contained type via Deref
    _value: &'b mut T,
    _borrow: BorrowRefMut<'b>,
}


impl<'b, T: ?Sized> Deref for RefMut<'b, T> {
    type Target = T;

    #[inline]
    fn deref<'a>(&'a self) -> &'a T {
        self._value
    }
}


impl<'b, T: ?Sized> DerefMut for RefMut<'b, T> {
    #[inline]
    fn deref_mut<'a>(&'a mut self) -> &'a mut T {
        self._value
    }
}
