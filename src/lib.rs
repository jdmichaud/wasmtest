#![no_std]

#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
  core::arch::wasm32::unreachable()
}

mod allocator;

#[global_allocator]
static ALLOCATOR: allocator::WasmAllocator = allocator::WasmAllocator::new();

#[no_mangle]
pub extern "C" fn add(lhs: u8, rhs: u8) -> i16 {
  (lhs as i16).wrapping_add(rhs as i16)
}

extern crate alloc;
use core::alloc::Layout;
use core::ptr;

#[no_mangle]
pub extern "C" fn foo() {
  let layout = Layout::array::<u8>(5).unwrap();
  let ptr = unsafe { alloc::alloc::alloc(layout) };
  unsafe { ptr::write(ptr.add(3), 42); }
}

