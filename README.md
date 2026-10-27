# mimalloc-ng

A Rust global allocator backed by [mimalloc](https://github.com/microsoft/mimalloc).

```rust
use mimalloc_ng::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;
```

Building needs a C compiler, plus a C++ compiler on MSVC targets. The mimalloc sources
are vendored, and can be found in `mimalloc-ng-sys/mimalloc`.

Beyond the global allocator, `mimalloc_ng::heap::Heap` exposes first-class heaps
that any thread can allocate from, `arena::Arena` reserves OS memory up front, and
`option::{Flag, Size, Number}` read and set runtime options, with sizes in bytes.
With `allocator_api`, `Vec::new_in(&heap)` allocates from a heap of your own.

## Features

| Feature                | Effect                                                                  |
| ---------------------- | ----------------------------------------------------------------------- |
| `secure`               | Guard pages, randomized allocation, encrypted free lists, ~10% slower   |
| `override`             | Replaces the C `malloc` family for the whole process                    |
| `debug`                | mimalloc's full assertions and invariant checks                         |
| `local_dynamic_tls`    | Local-dynamic TLS model, for libraries loaded with `dlopen`             |
| `win_direct_tls`       | Direct TLS slots only on Windows, if the process uses fewer than 64     |
| `allocator_api`        | Implements `core::alloc::Allocator`, needs Rust 1.100                   |
| `free_use_pagemap`     | Lifts the ~256 MiB arena object limit, `free` does a page map lookup    |
| `skip_collect_on_exit` | Skips freeing all memory at process exit                                |
| `profiler` (default)   | Heap profiler with stack traces, written when `MIMALLOC_PROFILE` is set |

Runtime tuning goes through mimalloc's own options, via either the `MIMALLOC_*`
environment variables or the `option` module.

As an example, `MIMALLOC_ALLOW_THP=0` turns off transparent huge pages, and
`MIMALLOC_PROFILE=heap` writes pprof heap profiles to `heap.*.pb`.

## Updating mimalloc

```sh
nu mimalloc-ng-sys/update-mimalloc.nu v3.6.0
cargo test --workspace
```

The `abi` test checks every binding in `mimalloc-ng-sys` against the new headers.

## License

mimalloc-ng is licensed under the MIT license.
