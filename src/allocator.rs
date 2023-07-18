use core::alloc::GlobalAlloc;
use core::alloc::Layout;

#[link(wasm_import_module = "env")]
extern "C" {
  fn malloc(size: usize) -> *mut u8;
}

#[repr(C, align(32))]
pub struct WasmAllocator {}

impl WasmAllocator {
  pub const fn new() -> Self {
    WasmAllocator {}
  }
}

unsafe impl Sync for WasmAllocator {}

unsafe impl GlobalAlloc for WasmAllocator {
  unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
    let size = layout.size();
    malloc(size)
  }

  unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}
