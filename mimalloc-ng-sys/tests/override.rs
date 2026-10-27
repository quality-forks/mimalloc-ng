//! With `override`, the C `malloc` family resolves to mimalloc, including calls
//! made inside other shared objects such as libc.

#![cfg(all(feature = "override", unix))]

use mimalloc_ng_sys::{
   mi_any_heap_contains,
   mi_free,
   mi_malloc,
};

#[test]
fn malloc_is_mimalloc() {
   // SAFETY: any size is valid input.
   let ptr = unsafe { libc::malloc(16) };
   assert!(!ptr.is_null(), "malloc returned null");

   // SAFETY: `ptr` is a live block.
   let owned = unsafe { mi_any_heap_contains(ptr) };
   assert!(owned, "malloc did not allocate from mimalloc");

   // SAFETY: `ptr` is a mimalloc block.
   unsafe { mi_free(ptr) }
}

#[test]
fn libc_allocates_from_mimalloc() {
   // SAFETY: the argument is a nul-terminated string. `strdup` calls `malloc`
   // from inside libc.
   let ptr = unsafe { libc::strdup(c"mimalloc".as_ptr()) };
   assert!(!ptr.is_null(), "strdup returned null");

   // SAFETY: `ptr` is a live block.
   let owned = unsafe { mi_any_heap_contains(ptr.cast()) };
   assert!(owned, "libc's malloc did not resolve to mimalloc");

   // SAFETY: `ptr` is a mimalloc block.
   unsafe { mi_free(ptr.cast()) }
}

#[test]
fn free_accepts_mi_malloc() {
   // SAFETY: any size is valid input.
   let ptr = unsafe { mi_malloc(16) };
   assert!(!ptr.is_null(), "mi_malloc returned null");

   // SAFETY: `free` is `mi_free` here, so it accepts a mimalloc block.
   unsafe { libc::free(ptr) }
}
