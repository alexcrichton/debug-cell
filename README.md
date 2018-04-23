# debug-cell

[![Build Status](https://travis-ci.org/alexcrichton/debug-cell.svg?branch=master)](https://travis-ci.org/alexcrichton/debug-cell)

[Documentation](https://docs.rs/debug-cell)

A clone of the standard library's `RefCell` type with extra debugging support in
non-release builds. Whenever a borrow error happens the current locations of
where known borrows were created will be printed out as well.

# License

This project is licensed under either of

 * Apache License, Version 2.0, ([LICENSE-APACHE](LICENSE-APACHE) or
   http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license ([LICENSE-MIT](LICENSE-MIT) or
   http://opensource.org/licenses/MIT)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in Serde by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
