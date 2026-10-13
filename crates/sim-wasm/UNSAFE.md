# Unsafe code in sim-wasm

`crates/sim-wasm/src/abi.rs` is the only module allowed to use `unsafe` (`#![allow(unsafe_code)]`
at module level; the crate otherwise has `unsafe_code = "deny"`). `tests/no_alloc.rs` also allows it
for a counting allocator; that file is test-only.

## What is unsafe

1. `#[unsafe(no_mangle)]` on the exported `extern "C"` functions. Required to export symbols.
2. One global `static STATE: Global`, an `UnsafeCell<State>` wrapped in a type that implements
   `Sync` by assertion. `State` owns the core, the content buffer and the input staging buffer.
3. Dereferencing the cell inside `with_state`.
4. `unsafe extern "C"` around the one import, `env.fg_now` (the profiler's clock), on `wasm32`
   only. The function itself is declared `safe`: it takes nothing and returns a number.

Handing out addresses is not unsafe: `slice.as_ptr() as u32` is ordinary Rust. Reading through
them happens in JavaScript, which is why the rules below exist.

## Invariants

- **Single caller.** All `fg_*` functions are called from one thread only: the Sim Worker
  (ADR-17). The threads build shares *memory* with other workers, but only the Sim Worker calls
  into this module. No function is re-entrant: `with_state` closures never call another `fg_*`
  function.
- **No references escape.** `with_state` hands a `&mut State` to a closure and drops it on
  return; nothing keeps a reference past the call.
- **Panic is abort.** The release profile sets `panic = "abort"`, so a panic traps instead of
  unwinding through the FFI boundary with the cell borrowed.

## Pointers handed to the host

The buffers behind `fg_content_reserve`, `fg_input_ptr`, `fg_snapshot_ptr`, `fg_profile_ptr` and
`fg_terrain_ptr` live in linear memory, so the host sees them as offsets into `memory.buffer`.

- **Lifetime.** Each address is valid until the call named in the table at the top of `abi.rs`
  (in short: the input address never changes; the snapshot, profile and terrain addresses
  change on `fg_init`, because the core is replaced; the content address changes on
  `fg_content_reserve`, `fg_init` and a successful `fg_load_content`).
- **Views.** Any call that allocates may grow memory, and growing replaces the `ArrayBuffer`
  (or `SharedArrayBuffer`) behind every typed-array view. After `fg_init`, `fg_content_reserve`,
  `fg_load_content` or the first `fg_terrain_fill`, the host re-derives its views from
  `memory.buffer`. `fg_step`, `fg_push_inputs` and the pointer getters never allocate, so views
  taken after those stay valid across a whole run of ticks. This is tested natively with a
  counting allocator (`tests/no_alloc.rs`) and checked again in the browser (the sim host compares
  `memory.buffer` identity before every read).
- **Who writes.** The host writes only the content buffer and the input staging buffer, and only
  between calls. The core writes the snapshot and terrain buffers, only inside `fg_step`,
  lifecycle calls and `fg_terrain_fill`; `fg_profile_ptr` refreshes the profile block before
  returning. The host reads a buffer only on the Sim Worker thread, between calls, and copies
  the snapshot out before another worker sees it.
- **No concurrent access.** In the threads build other workers can *address* the same memory,
  but nothing but the Sim Worker touches these buffers. Cross-thread data goes through the
  seqlock snapshot buffer, which is a separate `SharedArrayBuffer`.

## If this changes

If a second thread ever needs to call the core, replace the global with a proper handle API or a
`Mutex`; do not weaken the invariants above.
