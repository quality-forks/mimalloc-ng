//! A Rust global allocator backed by [mimalloc](https://github.com/microsoft/mimalloc) 3.6.0.
//!
//! ```
//! use mimalloc_ng::MiMalloc;
//!
//! #[global_allocator]
//! static GLOBAL: MiMalloc = MiMalloc;
//! ```
//!
//! Cargo features map to mimalloc build options:
//!
//! - `secure` adds guard pages, randomized allocation, and encrypted free
//!   lists, at roughly 10% throughput.
//! - `override` replaces the C `malloc` family for the whole process.
//! - `debug` enables mimalloc's full assertions and invariant checks.
//! - `local_dynamic_tls` uses the local-dynamic TLS model, for code loaded with
//!   `dlopen`.
//! - `win_direct_tls` uses only direct TLS slots on Windows.
//! - `allocator_api` implements `core::alloc::Allocator`, stable since Rust
//!   1.100.
//!
//! The raw C API lives in [`mimalloc_ng_sys`].

#![no_std]

/// `core::alloc::Allocator` for [`MiMalloc`].
#[cfg(feature = "allocator_api")]
#[clippy::msrv = "1.100"]
mod allocator_api;

use core::{
   alloc::{
      GlobalAlloc,
      Layout,
   },
   ffi::{
      CStr,
      c_char,
      c_int,
   },
   ops::Deref,
   ptr::{
      self,
      NonNull,
   },
};

use mimalloc_ng_sys::{
   mi_free,
   mi_malloc_aligned,
   mi_realloc_aligned,
   mi_stats_get_json,
   mi_usable_size,
   mi_version,
   mi_zalloc_aligned,
};

/// The mimalloc global allocator.
#[derive(Clone, Copy, Debug, Default)]
pub struct MiMalloc;

// SAFETY: every method forwards to the mimalloc function with the same
// contract, and mimalloc returns null on failure.
unsafe impl GlobalAlloc for MiMalloc {
   #[inline]
   unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
      // SAFETY: any size and power-of-two alignment is valid input.
      unsafe { mi_malloc_aligned(layout.size(), layout.align()) }.cast()
   }

   #[inline]
   unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
      // SAFETY: any size and power-of-two alignment is valid input.
      unsafe { mi_zalloc_aligned(layout.size(), layout.align()) }.cast()
   }

   #[inline]
   unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
      // SAFETY: the caller guarantees `ptr` came from this allocator.
      unsafe { mi_free(ptr.cast()) }
   }

   #[inline]
   unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
      // SAFETY: the caller guarantees `ptr` came from this allocator with
      // `layout`.
      unsafe { mi_realloc_aligned(ptr.cast(), new_size, layout.align()) }.cast()
   }
}

impl MiMalloc {
   /// Returns the runtime mimalloc version as `major * 10000 + minor * 100 +
   /// patch`.
   ///
   /// ```
   /// assert_eq!(
   ///    mimalloc_ng::MiMalloc.version(),
   ///    mimalloc_ng_sys::MI_MALLOC_VERSION
   /// );
   /// ```
   #[inline]
   #[must_use = "this only reads the version"]
   pub fn version(&self) -> c_int {
      // SAFETY: takes no input and only reads a constant.
      unsafe { mi_version() }
   }

   /// Returns the usable size of the block at `ptr`, at least the size it was
   /// allocated with.
   ///
   /// # Safety
   ///
   /// `ptr` must be null or point to a live block allocated by mimalloc.
   #[inline]
   #[must_use = "this only reads the block size"]
   pub unsafe fn usable_size(&self, ptr: *const u8) -> usize {
      // SAFETY: the caller guarantees `ptr` is null or a live mimalloc block.
      unsafe { mi_usable_size(ptr.cast()) }
   }

   /// Returns the process-wide statistics as JSON, or `None` if mimalloc is out
   /// of memory.
   ///
   /// ```
   /// let stats = mimalloc_ng::MiMalloc.stats_json().unwrap();
   /// assert!(stats.to_bytes().starts_with(b"{"));
   /// ```
   #[inline]
   #[must_use = "the string is freed when dropped"]
   pub fn stats_json(&self) -> Option<StatsJson> {
      // SAFETY: a null buffer makes mimalloc allocate the result itself.
      NonNull::new(unsafe { mi_stats_get_json(0, ptr::null_mut()) }).map(StatsJson)
   }
}

/// A nul-terminated JSON string allocated by mimalloc, freed on drop.
#[derive(Debug)]
pub struct StatsJson(NonNull<c_char>);

impl Deref for StatsJson {
   type Target = CStr;

   #[inline]
   fn deref(&self) -> &CStr {
      // SAFETY: mimalloc returns a nul-terminated string that lives until
      // `drop`.
      unsafe { CStr::from_ptr(self.0.as_ptr()) }
   }
}

impl Drop for StatsJson {
   #[inline]
   fn drop(&mut self) {
      // SAFETY: the string was allocated by mimalloc and is freed exactly once.
      unsafe { mi_free(self.0.as_ptr().cast()) }
   }
}
