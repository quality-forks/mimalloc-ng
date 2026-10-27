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
//! - `free_use_pagemap` lifts the ~256 MiB limit on objects in arenas, at the
//!   cost of a page map lookup in every `free`.
//! - `skip_collect_on_exit` skips freeing all memory at process exit.
//! - `profiler`, on by default, builds mimalloc's heap profiler with stack
//!   traces. It writes pprof files when `MIMALLOC_PROFILE` is set.
//! - `allocator_api` implements `core::alloc::Allocator`, stable since Rust
//!   1.100.
//!
//! [`heap::Heap`] gives first-class heaps, [`arena::Arena`] reserves OS memory
//! up front, and [`option`] reads and sets runtime options. The raw C API lives
//! in [`mimalloc_ng_sys`].

#![no_std]

/// `core::alloc::Allocator` for [`MiMalloc`] and [`heap::Heap`].
#[cfg(feature = "allocator_api")]
#[clippy::msrv = "1.100"]
mod allocator_api;
pub mod arena;
pub mod heap;
pub mod option;

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
   fmt,
   ops::Deref,
   ptr::{
      self,
      NonNull,
   },
};

use mimalloc_ng_sys::{
   MI_MALLOC_VERSION,
   mi_collect,
   mi_free,
   mi_malloc_aligned,
   mi_realloc_aligned,
   mi_stats_get,
   mi_stats_get_json,
   mi_stats_t,
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

/// A mimalloc release.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
   /// Major version.
   pub major: u32,
   /// Minor version.
   pub minor: u32,
   /// Patch version.
   pub patch: u32,
}

impl Version {
   /// The version of the mimalloc headers these bindings were written against.
   pub const HEADER: Self = Self::from_raw(MI_MALLOC_VERSION);

   /// Splits mimalloc's `major * 10000 + minor * 100 + patch` encoding.
   const fn from_raw(raw: c_int) -> Self {
      let encoded = raw.unsigned_abs();

      Self {
         major: encoded / 10000,
         minor: encoded / 100 % 100,
         patch: encoded % 100,
      }
   }
}

impl fmt::Display for Version {
   #[inline]
   #[expect(
      clippy::renamed_function_params,
      reason = "the trait's `f` is shorter than the two-character minimum"
   )]
   fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(formatter, "{}.{}.{}", self.major, self.minor, self.patch)
   }
}

/// Returns the version of the linked mimalloc.
///
/// ```
/// use mimalloc_ng::Version;
///
/// assert_eq!(mimalloc_ng::version(), Version::HEADER);
/// assert_eq!(Version::HEADER.to_string(), "3.6.0");
/// ```
#[inline]
#[must_use = "this only reads the version"]
pub fn version() -> Version {
   // SAFETY: takes no input and only reads a constant.
   Version::from_raw(unsafe { mi_version() })
}

/// Returns free memory held by the calling thread to the OS. `force` also
/// collects abandoned and cached memory.
#[inline]
pub fn collect(force: bool) {
   // SAFETY: only touches the calling thread's heap.
   unsafe { mi_collect(force) }
}

/// Returns the usable size of the block at `ptr`, at least the size it was
/// allocated with.
///
/// # Safety
///
/// `ptr` must be null or point to a live block allocated by mimalloc.
#[inline]
#[must_use = "this only reads the block size"]
pub unsafe fn usable_size<T>(ptr: *const T) -> usize
where
   T: ?Sized,
{
   // SAFETY: the caller guarantees `ptr` is null or a live mimalloc block.
   unsafe { mi_usable_size(ptr.cast()) }
}

/// Returns the process-wide statistics, or `None` if mimalloc rejects the size
/// or version of `mi_stats_t`.
///
/// Allocation counters such as `malloc_count` are merged from each page in
/// batches, so they lag until [`collect`] runs.
///
/// ```
/// let stats = mimalloc_ng::stats().unwrap();
/// assert!(stats.reserved.current > 0);
/// ```
#[inline]
#[must_use = "this only reads the statistics"]
pub fn stats() -> Option<mi_stats_t> {
   let mut stats = mi_stats_t::new();

   // SAFETY: `stats` carries its own size and version.
   unsafe { mi_stats_get(&raw mut stats) }.then_some(stats)
}

/// Returns the process-wide statistics as JSON, or `None` if mimalloc is out of
/// memory.
///
/// ```
/// let stats = mimalloc_ng::stats_json().unwrap();
/// assert!(stats.starts_with('{'));
/// ```
#[inline]
#[must_use = "the string is freed when dropped"]
pub fn stats_json() -> Option<StatsJson> {
   // SAFETY: a null buffer makes mimalloc allocate the result itself.
   let json = unsafe { mi_stats_get_json(0, ptr::null_mut()) };

   // SAFETY: mimalloc returns null or a string allocated with `mi_malloc`.
   unsafe { StatsJson::from_raw(json) }
}

/// A JSON string allocated by mimalloc, freed on drop.
#[derive(Debug)]
pub struct StatsJson(NonNull<str>);

// SAFETY: the string is immutable and `mi_free` accepts blocks from any thread.
unsafe impl Send for StatsJson {}

// SAFETY: as above, shared access only reads the string.
unsafe impl Sync for StatsJson {}

impl StatsJson {
   /// Takes ownership of a JSON string returned by mimalloc. Returns `None` for
   /// null, and frees the string and returns `None` if it is not UTF-8.
   ///
   /// # Safety
   ///
   /// `raw` must be null or a nul-terminated string allocated by mimalloc and
   /// owned by the caller.
   unsafe fn from_raw(raw: *mut c_char) -> Option<Self> {
      let block = NonNull::new(raw)?;

      // SAFETY: the caller guarantees a nul-terminated string.
      let bytes = unsafe { CStr::from_ptr(block.as_ptr()) };

      if let Ok(text) = bytes.to_str() {
         return Some(Self(NonNull::from(text)));
      }

      // SAFETY: the caller hands over ownership of the mimalloc block.
      unsafe { mi_free(block.as_ptr().cast()) }
      None
   }
}

impl Deref for StatsJson {
   type Target = str;

   #[inline]
   fn deref(&self) -> &str {
      // SAFETY: the string is valid UTF-8 and lives until `drop`.
      unsafe { self.0.as_ref() }
   }
}

impl fmt::Display for StatsJson {
   #[inline]
   #[expect(
      clippy::renamed_function_params,
      reason = "the trait's `f` is shorter than the two-character minimum"
   )]
   fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
      formatter.write_str(self)
   }
}

impl Drop for StatsJson {
   #[inline]
   fn drop(&mut self) {
      // SAFETY: the string was allocated by mimalloc and is freed exactly once.
      unsafe { mi_free(self.0.as_ptr().cast()) }
   }
}
