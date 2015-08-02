extern crate debug_cell;

fn main() {
    let a = debug_cell::RefCell::new(4);
    let _c = a.borrow_mut();
    let _a = a.borrow();
    let _b = a.borrow();
}
