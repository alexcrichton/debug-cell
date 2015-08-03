# stacker

[![Build Status](https://travis-ci.org/alexcrichton/debug-cell.svg?branch=master)](https://travis-ci.org/alexcrichton/debug-cell)

[Documentation](http://alexcrichton.com/debug-cell)

A clone of the standard library's `RefCell` type with extra debugging support in
non-release builds. Whenever a borrow error happens the current locations of
where known borrows were created will be printed out as well.

# License

`debug-cell` is primarily distributed under the terms of both the MIT license and
the Apache License (Version 2.0), with portions covered by various BSD-like
licenses.

See LICENSE-APACHE, and LICENSE-MIT for details.
