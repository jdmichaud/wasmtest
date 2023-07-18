## Setup

Create the project:
```
cargo init --lib
```
We need lib here because a wasm binary is not an executable but a lib (no main).

Then we replace `lib.rs` with:
```rust
#[no_mangle]
pub extern "C" fn add(lhs: u8, rhs: u8) -> i16 {
  lhs as i16 + rhs
}
```
We need to compile this to wasm. Let's have a look at the target list:
```bash
rustc --print target-list | grep wasm
```
We want to compile for the browser, not wasi, so we choose: wasm32-unknown-unknown.

We need to install that target:
```bash
rustup target add wasm32-unknown-unknown
```
Then we compile:
```bash
cargo build --target wasm32-unknown-unknown
```
In order to diagnose wasm binary we need the Wasm Binary ToolKit. On ubuntu:
```bash
apt install -y wabt
```
The wasm library is in target:
```
└── wasm32-unknown-unknown
    └── debug
        ├── libwasmtest.d
        └── libwasmtest.rlib
```
But these are not what we want. We want an elf wasm file. We modify `Cargo.toml`:
```toml
[lib]
crate-type = ["cdylib"]
```
We build again and now:
```
└── wasm32-unknown-unknown
    └── debug
        ├── libwasmtest.d
        ├── libwasmtest.rlib
        ├── wasmtest.d
        └── wasmtest.wasm
```

## Tools

We can now convert the library to wither the WAT (WAsm Text) format:
```bash
wasm2wat target/wasm32-unknown-unknown/debug/wasmtest.wasm
```
or decompile to a more readable format:
```bash
wasm-decompile --enable-all target/wasm32-unknown-unknown/debug/wasmtest.wasm
```

## no_std

We do not want to depend on `std` to avoid having to link to libc and be able to provide our own allocator, so we add this
to `lib.rs`:
```rust
#![no_std)]
```

It means that a `panic_handler` is not provided. We have to provide our own:
```rust
#[panic_handler]
fn panic(_panic: &core::panic::PanicInfo<'_>) -> ! {
  core::arch::wasm32::unreachable()
}
```

In `no_std` mode, you can't access `std::`. Most of what you have in `std::` will be in `core::` or `alloc::`.
To access the alloc crate, you need to add in `lib.rs`:
```rust
extern crate alloc;
```

## Configuring the build

The library generated is very messy.
Decompiling it leads to more than 2500 lines of code. Most of it is related to managing panic when adding two integer.

Let's simplify:
```rust
#[no_mangle]
pub extern "C" fn add(lhs: u8, rhs: u8) -> i16 {
  (lhs as i16).wrapping_add(rhs as i16)
}
```
Here we wrap the addition so no panicking. Decompiling the library now gives 26 lines:
```wasm
export memory memory(initial: 16, max: 0);

global stack_pointer:int = 1048576;
export global data_end:int = 1048576;
export global heap_base:int = 1048576;

table T_a:funcref(min: 1, max: 1);

export function add(a:int, b:int):int {
  var c:int = stack_pointer;
  var d:int = 16;
  var e:int = c - d;
  e[10]:byte = a;
  e[11]:byte = b;
  var f:int = 255;
  var g:int = a & f;
  var h:int = b & f;
  e[6]:short = g;
  e[7]:short = h;
  var i:int = g + h;
  var j:int = 16;
  var k:int = i << j;
  var l:int = k >> j;
  return l;
}
```
First line seems to indicate that we are exporting memory.
Let's assume we want the memory to be entirely managed in the javascript world. We need to import memory.
> wasmtest.instance.exports.foo()
malloc 20 -> 0x100010
undefined
> wasmtest.instance.exports.foo()
malloc 20 -> 0x100024
undefined
> wasmtest.instance.exports.foo()
malloc 20 -> 0x100038
undefined

Add `.cargo/config.toml`:
```toml
[target.wasm32-unknown-unknown]
rustflags = [
  "-C", "link-args=--import-memory",
]
```
Once we compiled again, the first we decompile is:
```wasm
import memory env_memory;
```

# Memory

Now that memory management responsibilities are handed to javascript,
we need an allocator that will allocate memory through javascript, in `lib.rs` add:
```rust
mod allocator;

#[global_allocator]
static ALLOCATOR: allocator::WasmAllocator = allocator::WasmAllocator::new();
```
We need to create [`src/allocator.rs`](https://gist.githubusercontent.com/jdmichaud/9f4c07af6d1024d778b32bca3244623e/raw/allocator.rs).

Note that allocator calls `malloc` which is an external function provided from the javascript environment and never calls free.
We'll see later how to set that up.

Right now the allocator is not used because we don't dynamically allocate memory.

## Dynamic allocation

Let's create an artificial function that allocates memory in `lib.rs`:
```rustc
extern crate alloc;
use core::alloc::Layout;
use core::ptr;

#[no_mangle]
pub extern "C" fn foo() {
  let layout = Layout::array::<u8>(5).unwrap();
  let ptr = unsafe { alloc::alloc::alloc(layout) };
  unsafe { ptr::write(ptr.add(3), 42); }
}
```
This function does nothing but allocate memory and set a value in it (42 at index 3).
It uses the `alloc` crate that we need to declare as extern [for some reason](https://github.com/rust-lang/rust/issues/54392).

If you compile this you will see with `wasm-decompile` that suddenly a lot of cruft is added to the library binary.
Let's force rust to clean after itself by adding this in `Cargo.toml`:
```toml
[profile.dev]
opt-level="s" # optimize for size
```

After this, we get:
```wasm
import memory env_memory;

global stack_pointer:int = 1048576;
export global data_end:int = 1048577;
export global heap_base:int = 1048592;

table T_a:funcref(min: 1, max: 1);

data bss(offset: 1048576) = "\00";

import function ZN8wasmtest9allocator6malloc17hcb045658ead6b3afE(a:int):int;

export function add(a:int, b:int):int {
  return b + a
}

export function foo() {
  bss[0]:ubyte;
  ZN8wasmtest9allocator6malloc17hcb045658ead6b3afE(5)[3]:byte = 42;
}
```

We import a function called `ZN8wasmtest9allocator6malloc17hcb045658ead6b3afE` (as imported in `allocator.rs`) and we use it to allocate the array.

## Javascript

None of this will work if the javascript environment does not provide:
- A memory
- A malloc function.

Let's create `test.js` that will load the wasm module:
```javascript
const fs = require('fs');

let memory;
async function initWasm(path) {
  // Let's allocate 17 64K pages. Seems to be the minimal out example need.
  memory = new WebAssembly.Memory({ initial: 17 });

  // Position in memory of the next available free byte.
  // malloc will move that position.
  // Will be initialized by the __heap_base value once the wasm code is loaded.
  let heapPos;
  // This is the environment shared with the rust code.
  const env = {
    memory,
    // libc malloc reimplementation
    // This dumb allocator just churn through the memory and does not keep
    // track of freed memory.
    malloc: size => {
      const ptr = heapPos;
      heapPos += size;
      console.log('malloc', size, `-> 0x${ptr.toString(16)}`);
      return ptr;
    },
    // __assert_fail_js: (assertion, file, line, fun) => {
    //   const charArray = new Uint8Array(memory.buffer);
    //   console.log(`${toStr(charArray, file)}(${line}): ${toStr(charArray, assertion)} in ${toStr(charArray, fun)}`);
    // },
  }
  // Load the wasm code
  const buffer = fs.readFileSync(path);
  const wasmtest = await WebAssembly.instantiate(buffer, { env });
  // The rust code tell us where the __heap_base starts.
  heapPos = wasmtest.instance.exports.__heap_base.value;

  return wasmtest;
}

let wasmtest;
async function main() {
  wasmtest = await initWasm('target/wasm32-unknown-unknown/debug/wasmtest.wasm');
  console.log(wasmtest);
}

main();
```

Run it with `node test.js`. It should print the wasm module text representation.

Now you can also start it and keep a REPL running with:
```bash
node -i -e "$(< test.js)"
```

In the command line, type:
```bash
wasmtest.instance.exports.add(1, 2)
```

You get a 3. You called the `add` function. But that function does not allocate any memory though.
Let's try foo:
```bash
> wasmtest.instance.exports.foo()
malloc 5 -> 0x100010
undefined
> wasmtest.instance.exports.foo()
malloc 5 -> 0x100015
undefined
> wasmtest.instance.exports.foo()
malloc 5 -> 0x10001a
undefined
```
You see that 20 bytes are allocated and the address keeps increasing. `undefined` just means the function returned nothing.

Now look at the memory:
```bash
> array = new Uint8Array(memory.buffer)
[...]
> array[0x100010 + 3]
42
```
