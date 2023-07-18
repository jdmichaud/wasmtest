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

