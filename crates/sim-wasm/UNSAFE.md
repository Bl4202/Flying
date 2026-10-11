# Unsafe code in sim-wasm

`crates/sim-wasm/src/abi.rs` is the only module allowed to use `unsafe` (`#![allow(unsafe_code)]`
at module level; the crate otherwise has `unsafe_code = "deny"`).

## What is unsafe

1. `#[unsafe(no_mangle)]` on the exported `extern "C"` functions. Required to export symbols.
2. One global `static CORE: Global`, an `UnsafeCell<Option<SimCore>>` wrapped in a type that
   implements `Sync` by assertion.
3. Dereferencing the cell inside `with_core`.

## Invariants

- **Single caller.** All `fg_*` functions are called from one thread only: the Sim Worker
  (ADR-17). The threads build shares *memory* with other workers, but only the Sim Worker calls
  into this module. No function is re-entrant: `with_core` closures never call another `fg_*`
  function.
- **No references escape.** `with_core` hands a `&mut Option<SimCore>` to a closure and drops it
  on return; nothing keeps a reference past the call.
- **Panic is abort.** The release profile sets `panic = "abort"`, so a panic traps instead of
  unwinding through the FFI boundary with the cell borrowed.

## If this changes

If a second thread ever needs to call the core, replace the global with a proper handle API or a
`Mutex`; do not weaken the invariants above.
