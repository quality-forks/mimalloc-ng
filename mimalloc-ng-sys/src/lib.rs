//! Raw FFI bindings to [mimalloc](https://github.com/microsoft/mimalloc) 3.6.0.
//!
//! - The C library is compiled from the vendored sources in `mimalloc/` by
//!   `build.rs` as a single translation unit (`static.c`) and linked
//!   statically.
//! - Every item mirrors `mimalloc.h` and `mimalloc-stats.h` one to one, and the
//!   `abi` test crate checks signatures, constants, and struct layouts against
//!   those headers.
//! - Deprecated entry points, the C++ `mi_new` wrappers, and the wide-character
//!   helpers are not bound.
//! - Memory from any allocation function here is released with [`mi_free`],
//!   heaps included.

#![no_std]
#![expect(
   non_camel_case_types,
   non_upper_case_globals,
   reason = "names mirror the C headers"
)]

use core::{
   ffi::{
      c_char,
      c_int,
      c_long,
      c_ulonglong,
      c_void,
   },
   marker::{
      PhantomData,
      PhantomPinned,
   },
   mem,
};

/// Version of the vendored mimalloc, as `major * 10000 + minor * 100 + patch`.
pub const MI_MALLOC_VERSION: c_int = 30600;

/// Largest size in machine words accepted by the `_small` allocation functions.
pub const MI_SMALL_WSIZE_MAX: usize = 128;

/// Largest size in bytes accepted by the `_small` allocation functions.
pub const MI_SMALL_SIZE_MAX: usize = MI_SMALL_WSIZE_MAX * size_of::<usize>();

/// Version of the [`mi_stats_t`] layout, bumped on every incompatible change.
pub const MI_STAT_VERSION: usize = 6;

/// Number of size bins in [`mi_stats_t::malloc_bins`] and
/// [`mi_stats_t::page_bins`], minus one.
pub const MI_BIN_HUGE: usize = 73;

/// A first-class heap. Any thread may allocate from it and free its blocks.
///
/// Heaps keep their blocks in separate pages but share arenas and freed pages.
#[repr(C)]
pub struct mi_heap_t {
   /// Private and zero-sized, so values only exist behind pointers from
   /// mimalloc.
   _data:   [u8; 0],
   /// Opts out of `Send`, `Sync`, and `Unpin`, which mimalloc does not promise.
   _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// A thread-local view of a heap. Only the creating thread may allocate from
/// it.
#[repr(C)]
pub struct mi_theap_t {
   /// Private and zero-sized, so values only exist behind pointers from
   /// mimalloc.
   _data:   [u8; 0],
   /// Opts out of `Send`, `Sync`, and `Unpin`, which mimalloc does not promise.
   _marker: PhantomData<(*mut u8, PhantomPinned)>,
}

/// Identifies an arena created by one of the `_ex` reservation functions.
pub type mi_arena_id_t = *mut c_void;

/// Identifies a subprocess, whose arenas never share memory with other
/// subprocesses.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct mi_subproc_id_t {
   /// Opaque handle owned by mimalloc.
   _mi_subproc_id: *mut c_void,
}

/// One area of a heap, holding blocks of a single size.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct mi_heap_area_t {
   /// Start of the blocks in this area.
   pub blocks:          *mut c_void,
   /// Bytes of virtual memory reserved for this area.
   pub reserved:        usize,
   /// Bytes currently available in this area.
   pub committed:       usize,
   /// Number of allocated blocks.
   pub used:            usize,
   /// Size in bytes of each block.
   pub block_size:      usize,
   /// Size in bytes of each block including padding and metadata.
   pub full_block_size: usize,
   /// Internal to mimalloc.
   pub reserved1:       *mut c_void,
}

/// Deferred free callback. `heartbeat` increases monotonically per thread.
pub type mi_deferred_free_fun =
   unsafe extern "C" fn(force: bool, heartbeat: c_ulonglong, arg: *mut c_void);

/// Output callback for messages and statistics. Must be thread safe.
pub type mi_output_fun = unsafe extern "C" fn(msg: *const c_char, arg: *mut c_void);

/// Error callback, called with an `errno` code after the message is printed.
pub type mi_error_fun = unsafe extern "C" fn(err: c_int, arg: *mut c_void);

/// Block visitor. Called once per area with a null `block`, then per block if
/// requested. Return `false` to stop.
pub type mi_block_visit_fun = unsafe extern "C" fn(
   heap: *const mi_heap_t,
   area: *const mi_heap_area_t,
   block: *mut c_void,
   block_size: usize,
   arg: *mut c_void,
) -> bool;

/// Heap visitor for [`mi_subproc_visit_heaps`]. Return `false` to stop.
pub type mi_heap_visit_fun = unsafe extern "C" fn(heap: *mut mi_heap_t, arg: *mut c_void) -> bool;

/// Commit or decommit callback for memory handed to [`mi_manage_memory`].
pub type mi_commit_fun_t = unsafe extern "C" fn(
   commit: bool,
   start: *mut c_void,
   size: usize,
   is_zero: *mut bool,
   user_arg: *mut c_void,
) -> bool;

/// A runtime option, one of the `mi_option_*` constants.
pub type mi_option_t = c_int;

/// Print error messages.
pub const mi_option_show_errors: mi_option_t = 0;
/// Print statistics on termination.
pub const mi_option_show_stats: mi_option_t = 1;
/// Print verbose messages.
pub const mi_option_verbose: mi_option_t = 2;
/// Eagerly commit arenas. 2 enables it only on overcommit systems (=2).
pub const mi_option_arena_eager_commit: mi_option_t = 4;
/// Decommit on purge (=1). 0 resets memory instead.
pub const mi_option_purge_decommits: mi_option_t = 5;
/// Allow large (2 or 4 MiB) OS pages, implies eager commit.
pub const mi_option_allow_large_os_pages: mi_option_t = 6;
/// Reserve N huge (1 GiB) OS pages at startup.
pub const mi_option_reserve_huge_os_pages: mi_option_t = 7;
/// Reserve the huge OS pages at a specific NUMA node.
pub const mi_option_reserve_huge_os_pages_at: mi_option_t = 8;
/// Reserve this much OS memory in an arena at startup, in KiB.
pub const mi_option_reserve_os_memory: mi_option_t = 9;
/// Delay purging by N milliseconds. 0 purges immediately, -1 never (=10).
pub const mi_option_purge_delay: mi_option_t = 15;
/// Use at most N NUMA nodes. 0 uses all.
pub const mi_option_use_numa_nodes: mi_option_t = 16;
/// Allocate only from reserved arenas, never directly from the OS.
pub const mi_option_disallow_os_alloc: mi_option_t = 17;
/// Tag for OS memory accounting, macOS only (=100).
pub const mi_option_os_tag: mi_option_t = 18;
/// Print at most N error messages.
pub const mi_option_max_errors: mi_option_t = 19;
/// Print at most N warning messages.
pub const mi_option_max_warnings: mi_option_t = 20;
/// Release all memory on exit. Can be unsafe.
pub const mi_option_destroy_on_exit: mi_option_t = 22;
/// Initial arena reservation size, in KiB (=1 GiB on 64-bit).
pub const mi_option_arena_reserve: mi_option_t = 23;
/// Multiplier of `purge_delay` for arenas (=10).
pub const mi_option_arena_purge_mult: mi_option_t = 24;
/// Allocate only from explicitly requested arenas.
pub const mi_option_disallow_arena_alloc: mi_option_t = 26;
/// Retry for N milliseconds on out-of-memory, Windows only (=400).
pub const mi_option_retry_on_oom: mi_option_t = 27;
/// Minimal rounded object size for guarded objects (=0).
pub const mi_option_guarded_min: mi_option_t = 29;
/// Maximal rounded object size for guarded objects (=0).
pub const mi_option_guarded_max: mi_option_t = 30;
/// Place guarded blocks exactly against the guard page, ignoring minimal
/// alignment (=0).
pub const mi_option_unsafe_guarded_precise: mi_option_t = 31;
/// Guard about one object per N requested bytes (=0).
pub const mi_option_guarded_sample_rate: mi_option_t = 32;
/// Seed for guarded sampling, for reproducible runs (=0).
pub const mi_option_guarded_sample_seed: mi_option_t = 33;
/// Collect theaps every N generic allocation calls (=10000).
pub const mi_option_generic_collect: mi_option_t = 34;
/// Default reclaim-on-free policy: -1 never, 0 originating theap, 1 any theap.
pub const mi_option_page_reclaim_on_free: mi_option_t = 35;
/// Retain N full small pages per size class (=2).
pub const mi_option_page_full_retain: mi_option_t = 36;
/// Consider at most N candidate pages per allocation (=4).
pub const mi_option_page_max_candidates: mi_option_t = 37;
/// Maximum user-space virtual address bits (=48).
pub const mi_option_max_vabits: mi_option_t = 38;
/// Commit the full page map to always catch invalid pointers (=0).
pub const mi_option_pagemap_commit: mi_option_t = 39;
/// Commit page memory on demand.
pub const mi_option_page_commit_on_demand: mi_option_t = 40;
/// Stop reclaiming same-theap pages once N are owned per size class (=-1,
/// unlimited).
pub const mi_option_page_max_reclaim: mi_option_t = 41;
/// Stop reclaiming cross-thread pages once N are owned per size class (=32).
pub const mi_option_page_cross_thread_max_reclaim: mi_option_t = 42;
/// Allow transparent huge pages (=1, Android =0).
pub const mi_option_allow_thp: mi_option_t = 43;
/// Minimal purge size in KiB (=0, meaning 64 or 2048 with THP).
pub const mi_option_arena_purge_min_size: mi_option_t = 44;
/// Maximal object size allocated in an arena, in KiB (=2 GiB on 64-bit).
pub const mi_option_arena_max_object_size: mi_option_t = 45;
/// Associate the local NUMA node with the initial arena. Experimental.
pub const mi_option_arena_is_numa_local: mi_option_t = 46;
/// Merge theap statistics into the parent heap on every collection.
pub const mi_option_stats_merge_on_collect: mi_option_t = 47;
/// Allocated bytes between automatic heap profiler snapshots (=1 GiB).
pub const mi_option_profile_alloc_interval: mi_option_t = 48;
/// In-use bytes grown between automatic heap profiler snapshots (=0, disabled).
pub const mi_option_profile_inuse_interval: mi_option_t = 49;
/// Seconds between automatic heap profiler snapshots (=0, disabled).
pub const mi_option_profile_time_interval: mi_option_t = 50;
/// Heap profiler sample rate in bytes (=512 KiB).
pub const mi_option_profile_sample_rate: mi_option_t = 51;
/// Merge theap statistics into the parent heap after N bytes change, 0 disables
/// (=512 KiB).
pub const mi_option_stats_merge_threshold: mi_option_t = 52;
/// Purge ranges above this size immediately.
pub const mi_option_arena_purge_immediate_size: mi_option_t = 53;

/// Statistic that tracks a running total, peak, and current value.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct mi_stat_count_t {
   /// Total allocated.
   pub total:   i64,
   /// Peak allocation.
   pub peak:    i64,
   /// Current allocation.
   pub current: i64,
}

/// Statistic that only increases.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct mi_stat_counter_t {
   /// Total count.
   pub total: i64,
}

/// Number of chunk size bins in [`mi_stats_t::chunk_bins`].
pub const MI_CBIN_COUNT: usize = 6;

/// Allocator statistics. Create with [`mi_stats_t::new`] so mimalloc can check
/// the size and version header.
#[repr(C, align(8))]
#[derive(Clone, Copy, Debug)]
pub struct mi_stats_t {
   pub size:    usize,
   pub version: usize,

   pub pages:            mi_stat_count_t,
   pub reserved:         mi_stat_count_t,
   pub committed:        mi_stat_count_t,
   pub reset:            mi_stat_counter_t,
   pub purged:           mi_stat_counter_t,
   pub page_committed:   mi_stat_count_t,
   pub pages_abandoned:  mi_stat_count_t,
   pub threads:          mi_stat_count_t,
   pub allocated:        mi_stat_count_t,
   pub malloc_count:     mi_stat_counter_t,
   pub free_count:       mi_stat_counter_t,
   pub freed:            mi_stat_counter_t,
   pub malloc_requested: mi_stat_counter_t,

   pub mmap_calls:           mi_stat_counter_t,
   pub commit_calls:         mi_stat_counter_t,
   pub reset_calls:          mi_stat_counter_t,
   pub purge_calls:          mi_stat_counter_t,
   pub arena_count:          mi_stat_counter_t,
   pub malloc_guarded_count: mi_stat_counter_t,

   pub arena_rollback_count: mi_stat_counter_t,
   pub arena_purges:         mi_stat_counter_t,
   pub pages_extended:       mi_stat_counter_t,
   pub pages_retire:         mi_stat_counter_t,
   pub page_searches:        mi_stat_counter_t,
   pub page_searches_count:  mi_stat_counter_t,

   pub segments:           mi_stat_count_t,
   pub segments_abandoned: mi_stat_count_t,
   pub segments_cache:     mi_stat_count_t,
   /// Reserved by mimalloc.
   pub segments_reserved:  mi_stat_count_t,

   pub heaps:                     mi_stat_count_t,
   pub theaps:                    mi_stat_count_t,
   pub pages_os_abandoned:        mi_stat_count_t,
   pub pages_os_allocated:        mi_stat_count_t,
   pub pages_reclaim_on_alloc:    mi_stat_counter_t,
   pub pages_reclaim_on_free:     mi_stat_counter_t,
   pub pages_reabandon_full:      mi_stat_counter_t,
   pub pages_unabandon_busy_wait: mi_stat_counter_t,
   pub pages_unown_cas_retry:     mi_stat_counter_t,
   pub heaps_delete_wait:         mi_stat_counter_t,
   pub pages_stat_updates:        mi_stat_counter_t,
   pub pages_stat_update_count:   mi_stat_counter_t,
   pub profile_samples:           mi_stat_counter_t,

   pub malloc_normal:       mi_stat_count_t,
   pub malloc_huge:         mi_stat_count_t,
   pub malloc_normal_count: mi_stat_counter_t,
   pub malloc_huge_count:   mi_stat_counter_t,

   /// Reserved by mimalloc.
   pub stat_reserved:         [mi_stat_count_t; 4],
   /// Reserved by mimalloc.
   pub stat_counter_reserved: [mi_stat_counter_t; 4],

   pub malloc_bins: [mi_stat_count_t; MI_BIN_HUGE + 1],
   pub page_bins:   [mi_stat_count_t; MI_BIN_HUGE + 1],
   pub chunk_bins:  [mi_stat_count_t; MI_CBIN_COUNT],
}

impl mi_stats_t {
   /// Zeroed statistics with the header set, like `mi_stats_init`.
   #[inline]
   #[must_use = "pass the result to a stats getter"]
   pub const fn new() -> Self {
      // SAFETY: every field is an integer or an array of integers, so all
      // zeroes is valid.
      let mut stats: Self = unsafe { mem::zeroed() };
      stats.size = size_of::<Self>();
      stats.version = MI_STAT_VERSION;
      stats
   }
}

impl Default for mi_stats_t {
   #[inline]
   fn default() -> Self {
      Self::new()
   }
}

unsafe extern "C" {
   /// Allocates `size` bytes. Returns null on out-of-memory and a unique
   /// pointer for size 0.
   pub fn mi_malloc(size: usize) -> *mut c_void;

   /// Allocates `count * size` zeroed bytes. Returns null on overflow or
   /// out-of-memory.
   pub fn mi_calloc(count: usize, size: usize) -> *mut c_void;

   /// Resizes `p` to `newsize` bytes, moving it if needed. On failure `p` stays
   /// valid and null is returned. A null `p` behaves like [`mi_malloc`].
   pub fn mi_realloc(p: *mut c_void, newsize: usize) -> *mut c_void;

   /// Resizes `p` in place. Returns null, leaving `p` untouched, if that would
   /// need a move.
   pub fn mi_expand(p: *mut c_void, newsize: usize) -> *mut c_void;

   /// Frees `p`, which must come from mimalloc or be null.
   pub fn mi_free(p: *mut c_void);

   /// Duplicates the nul-terminated string `s`.
   pub fn mi_strdup(s: *const c_char) -> *mut c_char;

   /// Duplicates at most `n` bytes of the nul-terminated string `s`.
   pub fn mi_strndup(s: *const c_char, n: usize) -> *mut c_char;

   /// Resolves `fname` to an absolute path. With a null `resolved_name` the
   /// result is allocated and must be freed with [`mi_free`], otherwise it is
   /// written to `resolved_name`, which must hold `PATH_MAX` bytes.
   pub fn mi_realpath(fname: *const c_char, resolved_name: *mut c_char) -> *mut c_char;

   /// Allocates at most [`MI_SMALL_SIZE_MAX`] bytes without checking the size.
   pub fn mi_malloc_small(size: usize) -> *mut c_void;

   /// Allocates at most [`MI_SMALL_SIZE_MAX`] zeroed bytes without checking the
   /// size.
   pub fn mi_zalloc_small(size: usize) -> *mut c_void;

   /// Allocates `size` zeroed bytes.
   pub fn mi_zalloc(size: usize) -> *mut c_void;

   /// Allocates `count * size` uninitialized bytes. Returns null on overflow.
   pub fn mi_mallocn(count: usize, size: usize) -> *mut c_void;

   /// Resizes `p` to `count * size` bytes. Returns null on overflow.
   pub fn mi_reallocn(p: *mut c_void, count: usize, size: usize) -> *mut c_void;

   /// Like [`mi_realloc`], but frees `p` when reallocation fails.
   pub fn mi_reallocf(p: *mut c_void, newsize: usize) -> *mut c_void;

   /// Returns the usable size of the block at `p`, at least its requested size.
   pub fn mi_usable_size(p: *const c_void) -> usize;

   /// Returns the size mimalloc would actually allocate for a request of `size`
   /// bytes.
   pub fn mi_good_size(size: usize) -> usize;

   /// Frees `p`, using `size` to pick the small-object path.
   pub fn mi_free_size(p: *mut c_void, size: usize);

   /// Frees a block from one of the `_small` allocation functions.
   pub fn mi_free_small(p: *mut c_void);

   /// Like [`mi_free_small`], but `p` must not be null.
   pub fn mi_free_small_nonnull(p: *mut c_void);

   /// Like [`mi_free_small`], but `p` must live on a page owned by the calling
   /// thread.
   pub fn mi_free_small_local(p: *mut c_void);

   /// Like [`mi_free_small_local`], but `p` must not be null.
   pub fn mi_free_small_local_nonnull(p: *mut c_void);

   /// Returns the id of the thread owning the page that holds `p`.
   pub fn mi_thread_id_of(p: *const c_void) -> usize;

   /// Allocates `size` bytes aligned to `alignment`.
   pub fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;

   /// Allocates `size` bytes such that `p + offset` is aligned to `alignment`.
   pub fn mi_malloc_aligned_at(size: usize, alignment: usize, offset: usize) -> *mut c_void;

   /// Allocates `size` zeroed bytes aligned to `alignment`.
   pub fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;

   /// Allocates `size` zeroed bytes such that `p + offset` is aligned to
   /// `alignment`.
   pub fn mi_zalloc_aligned_at(size: usize, alignment: usize, offset: usize) -> *mut c_void;

   /// Allocates `count * size` zeroed bytes aligned to `alignment`.
   pub fn mi_calloc_aligned(count: usize, size: usize, alignment: usize) -> *mut c_void;

   /// Allocates `count * size` zeroed bytes such that `p + offset` is aligned
   /// to `alignment`.
   pub fn mi_calloc_aligned_at(
      count: usize,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// Resizes `p` to `newsize` bytes aligned to `alignment`.
   pub fn mi_realloc_aligned(p: *mut c_void, newsize: usize, alignment: usize) -> *mut c_void;

   /// Resizes `p` to `newsize` bytes such that `p + offset` is aligned to
   /// `alignment`.
   pub fn mi_realloc_aligned_at(
      p: *mut c_void,
      newsize: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// Registers the deferred free callback, or unregisters it with `None`.
   pub fn mi_register_deferred_free(deferred_free: Option<mi_deferred_free_fun>, arg: *mut c_void);

   /// Registers the output callback, or restores stderr with `None`.
   pub fn mi_register_output(out: Option<mi_output_fun>, arg: *mut c_void);

   /// Registers the error callback. Codes are `EAGAIN` (double free), `EFAULT`
   /// (corrupted metadata), `ENOMEM`, `EOVERFLOW` (request too large), and
   /// `EINVAL` (invalid pointer).
   pub fn mi_register_error(fun: Option<mi_error_fun>, arg: *mut c_void);

   /// Returns free memory to the OS. `force` also collects abandoned and cached
   /// memory.
   pub fn mi_collect(force: bool);

   /// Returns the runtime mimalloc version, see [`MI_MALLOC_VERSION`].
   pub fn mi_version() -> c_int;

   /// Prints the current option values.
   pub fn mi_options_print();

   /// Prints process time and memory usage.
   pub fn mi_process_info_print();

   /// Prints the current option values through `out`.
   pub fn mi_options_print_out(out: Option<mi_output_fun>, arg: *mut c_void);

   /// Prints process time and memory usage through `out`.
   pub fn mi_process_info_print_out(out: Option<mi_output_fun>, arg: *mut c_void);

   /// Reads process time and memory usage. Every out-parameter may be null.
   pub fn mi_process_info(
      elapsed_msecs: *mut usize,
      user_msecs: *mut usize,
      system_msecs: *mut usize,
      current_rss: *mut usize,
      peak_rss: *mut usize,
      current_commit: *mut usize,
      peak_commit: *mut usize,
      page_faults: *mut usize,
   );

   /// Initializes the process. Normally called automatically.
   pub fn mi_process_init();

   /// Tears down the process. Normally called automatically.
   pub fn mi_process_done();

   /// Initializes the calling thread. Normally called automatically.
   pub fn mi_thread_init();

   /// Tears down the calling thread. Normally called automatically.
   pub fn mi_thread_done();

   /// Marks the calling thread as a thread pool worker.
   pub fn mi_thread_set_in_threadpool();

   /// Like [`mi_malloc`], also storing the block size in `block_size` on
   /// success.
   pub fn mi_umalloc(size: usize, block_size: *mut usize) -> *mut c_void;

   /// Like [`mi_calloc`], also storing the block size in `block_size` on
   /// success.
   pub fn mi_ucalloc(count: usize, size: usize, block_size: *mut usize) -> *mut c_void;

   /// Like [`mi_realloc`], also storing the old and new block sizes on success.
   pub fn mi_urealloc(
      p: *mut c_void,
      newsize: usize,
      block_size_pre: *mut usize,
      block_size_post: *mut usize,
   ) -> *mut c_void;

   /// Like [`mi_free`], also storing the freed block size in `block_size`.
   pub fn mi_ufree(p: *mut c_void, block_size: *mut usize);

   /// Like [`mi_malloc_aligned`], also storing the block size in `block_size`
   /// on success.
   pub fn mi_umalloc_aligned(size: usize, alignment: usize, block_size: *mut usize) -> *mut c_void;

   /// Like [`mi_zalloc_aligned`], also storing the block size in `block_size`
   /// on success.
   pub fn mi_uzalloc_aligned(size: usize, alignment: usize, block_size: *mut usize) -> *mut c_void;

   /// Like [`mi_malloc_small`], also storing the block size in `block_size` on
   /// success.
   pub fn mi_umalloc_small(size: usize, block_size: *mut usize) -> *mut c_void;

   /// Like [`mi_zalloc_small`], also storing the block size in `block_size` on
   /// success.
   pub fn mi_uzalloc_small(size: usize, block_size: *mut usize) -> *mut c_void;

   /// Creates a heap.
   pub fn mi_heap_new() -> *mut mi_heap_t;

   /// Deletes `heap`, moving its live blocks to the main heap.
   pub fn mi_heap_delete(heap: *mut mi_heap_t);

   /// Destroys `heap` and frees all its live blocks.
   pub fn mi_heap_destroy(heap: *mut mi_heap_t);

   /// Prefers memory from `numa_node` for `heap`.
   pub fn mi_heap_set_numa_affinity(heap: *mut mi_heap_t, numa_node: c_int);

   /// In debug builds, skips filling the first `preserve` bytes of blocks on
   /// allocation and free.
   pub fn mi_heap_set_debug_fill_preserve(heap: *mut mi_heap_t, preserve: usize);

   /// Returns free memory of `heap` to the OS.
   pub fn mi_heap_collect(heap: *mut mi_heap_t, force: bool);

   /// Returns the main heap.
   pub fn mi_heap_main() -> *mut mi_heap_t;

   /// Returns the heap holding `p`.
   pub fn mi_heap_of(p: *const c_void) -> *mut mi_heap_t;

   /// Returns whether `p` belongs to `heap`.
   pub fn mi_heap_contains(heap: *const mi_heap_t, p: *const c_void) -> bool;

   /// Returns whether `p` belongs to any heap.
   pub fn mi_any_heap_contains(p: *const c_void) -> bool;

   /// [`mi_malloc`] from `heap`.
   pub fn mi_heap_malloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void;

   /// [`mi_zalloc`] from `heap`.
   pub fn mi_heap_zalloc(heap: *mut mi_heap_t, size: usize) -> *mut c_void;

   /// [`mi_calloc`] from `heap`.
   pub fn mi_heap_calloc(heap: *mut mi_heap_t, count: usize, size: usize) -> *mut c_void;

   /// [`mi_mallocn`] from `heap`.
   pub fn mi_heap_mallocn(heap: *mut mi_heap_t, count: usize, size: usize) -> *mut c_void;

   /// [`mi_malloc_small`] from `heap`.
   pub fn mi_heap_malloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void;

   /// [`mi_zalloc_small`] from `heap`.
   pub fn mi_heap_zalloc_small(heap: *mut mi_heap_t, size: usize) -> *mut c_void;

   /// [`mi_realloc`] within `heap`.
   pub fn mi_heap_realloc(heap: *mut mi_heap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

   /// [`mi_reallocn`] within `heap`.
   pub fn mi_heap_reallocn(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      count: usize,
      size: usize,
   ) -> *mut c_void;

   /// [`mi_reallocf`] within `heap`.
   pub fn mi_heap_reallocf(heap: *mut mi_heap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

   /// [`mi_strdup`] from `heap`.
   pub fn mi_heap_strdup(heap: *mut mi_heap_t, s: *const c_char) -> *mut c_char;

   /// [`mi_strndup`] from `heap`.
   pub fn mi_heap_strndup(heap: *mut mi_heap_t, s: *const c_char, n: usize) -> *mut c_char;

   /// [`mi_realpath`] from `heap`.
   pub fn mi_heap_realpath(
      heap: *mut mi_heap_t,
      fname: *const c_char,
      resolved_name: *mut c_char,
   ) -> *mut c_char;

   /// [`mi_malloc_aligned`] from `heap`.
   pub fn mi_heap_malloc_aligned(
      heap: *mut mi_heap_t,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_malloc_aligned_at`] from `heap`.
   pub fn mi_heap_malloc_aligned_at(
      heap: *mut mi_heap_t,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// [`mi_zalloc_aligned`] from `heap`.
   pub fn mi_heap_zalloc_aligned(
      heap: *mut mi_heap_t,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_zalloc_aligned_at`] from `heap`.
   pub fn mi_heap_zalloc_aligned_at(
      heap: *mut mi_heap_t,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// [`mi_calloc_aligned`] from `heap`.
   pub fn mi_heap_calloc_aligned(
      heap: *mut mi_heap_t,
      count: usize,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_calloc_aligned_at`] from `heap`.
   pub fn mi_heap_calloc_aligned_at(
      heap: *mut mi_heap_t,
      count: usize,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// [`mi_realloc_aligned`] within `heap`.
   pub fn mi_heap_realloc_aligned(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newsize: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_realloc_aligned_at`] within `heap`.
   pub fn mi_heap_realloc_aligned_at(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newsize: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// Resizes `p` and zeroes the new bytes. `p` must come from a zeroing
   /// allocation.
   pub fn mi_rezalloc(p: *mut c_void, newsize: usize) -> *mut c_void;

   /// Resizes `p` to `newcount * size` bytes and zeroes the new bytes. `p` must
   /// come from a zeroing allocation.
   pub fn mi_recalloc(p: *mut c_void, newcount: usize, size: usize) -> *mut c_void;

   /// Aligned [`mi_rezalloc`].
   pub fn mi_rezalloc_aligned(p: *mut c_void, newsize: usize, alignment: usize) -> *mut c_void;

   /// Offset-aligned [`mi_rezalloc`].
   pub fn mi_rezalloc_aligned_at(
      p: *mut c_void,
      newsize: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// Aligned [`mi_recalloc`].
   pub fn mi_recalloc_aligned(
      p: *mut c_void,
      newcount: usize,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// Offset-aligned [`mi_recalloc`].
   pub fn mi_recalloc_aligned_at(
      p: *mut c_void,
      newcount: usize,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// [`mi_rezalloc`] within `heap`.
   pub fn mi_heap_rezalloc(heap: *mut mi_heap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

   /// [`mi_recalloc`] within `heap`.
   pub fn mi_heap_recalloc(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newcount: usize,
      size: usize,
   ) -> *mut c_void;

   /// [`mi_rezalloc_aligned`] within `heap`.
   pub fn mi_heap_rezalloc_aligned(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newsize: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_rezalloc_aligned_at`] within `heap`.
   pub fn mi_heap_rezalloc_aligned_at(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newsize: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// [`mi_recalloc_aligned`] within `heap`.
   pub fn mi_heap_recalloc_aligned(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newcount: usize,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_recalloc_aligned_at`] within `heap`.
   pub fn mi_heap_recalloc_aligned_at(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      newcount: usize,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// Visits every area of `heap`, and every block too if `visit_blocks`.
   /// Returns `true` if the visit completed. Assumes no concurrent access to
   /// `heap`.
   pub fn mi_heap_visit_blocks(
      heap: *mut mi_heap_t,
      visit_blocks: bool,
      visitor: Option<mi_block_visit_fun>,
      arg: *mut c_void,
   ) -> bool;

   /// Like [`mi_heap_visit_blocks`], but for pages abandoned by exited threads.
   pub fn mi_heap_visit_abandoned_blocks(
      heap: *mut mi_heap_t,
      visit_blocks: bool,
      visitor: Option<mi_block_visit_fun>,
      arg: *mut c_void,
   ) -> bool;

   /// Returns whether the C `malloc` family is redirected to mimalloc.
   pub fn mi_is_redirected() -> bool;

   /// Reserves `pages` huge OS pages spread over `numa_nodes` nodes, 0 meaning
   /// all. Returns 0 or an `errno` code.
   pub fn mi_reserve_huge_os_pages_interleave(
      pages: usize,
      numa_nodes: usize,
      timeout_msecs: usize,
   ) -> c_int;

   /// Reserves `pages` huge OS pages on `numa_node`. Returns 0 or an `errno`
   /// code.
   pub fn mi_reserve_huge_os_pages_at(
      pages: usize,
      numa_node: c_int,
      timeout_msecs: usize,
   ) -> c_int;

   /// Reserves `size` bytes of OS memory as an arena. Returns 0 or an `errno`
   /// code.
   pub fn mi_reserve_os_memory(size: usize, commit: bool, allow_large: bool) -> c_int;

   /// Hands the memory at `start` to mimalloc as an arena. `is_pinned` memory
   /// is never decommitted. `numa_node` may be -1.
   pub fn mi_manage_os_memory(
      start: *mut c_void,
      size: usize,
      is_committed: bool,
      is_pinned: bool,
      is_zero: bool,
      numa_node: c_int,
   ) -> bool;

   /// Prints the arenas, for debugging.
   pub fn mi_debug_show_arenas();

   /// Prints the arenas.
   pub fn mi_arenas_print();

   /// Purges unused memory from all arenas.
   pub fn mi_arenas_purge();

   /// Returns the required alignment of memory passed to
   /// [`mi_manage_os_memory`].
   pub fn mi_arena_min_alignment() -> usize;

   /// Returns the minimal size of an arena.
   pub fn mi_arena_min_size() -> usize;

   /// Returns the largest object size allocated in an arena.
   pub fn mi_arena_max_object_size() -> usize;

   /// Returns the start of `arena_id`, storing its size in `size`.
   pub fn mi_arena_area(arena_id: mi_arena_id_t, size: *mut usize) -> *mut c_void;

   /// [`mi_reserve_huge_os_pages_at`] that can make the arena `exclusive` and
   /// returns its id.
   pub fn mi_reserve_huge_os_pages_at_ex(
      pages: usize,
      numa_node: c_int,
      timeout_msecs: usize,
      exclusive: bool,
      arena_id: *mut mi_arena_id_t,
   ) -> c_int;

   /// [`mi_reserve_os_memory`] that can make the arena `exclusive` and returns
   /// its id.
   pub fn mi_reserve_os_memory_ex(
      size: usize,
      commit: bool,
      allow_large: bool,
      exclusive: bool,
      arena_id: *mut mi_arena_id_t,
   ) -> c_int;

   /// [`mi_manage_os_memory`] that can make the arena `exclusive` and returns
   /// its id.
   pub fn mi_manage_os_memory_ex(
      start: *mut c_void,
      size: usize,
      is_committed: bool,
      is_pinned: bool,
      is_zero: bool,
      numa_node: c_int,
      exclusive: bool,
      arena_id: *mut mi_arena_id_t,
   ) -> bool;

   /// Returns whether `p` lies in `arena_id`.
   pub fn mi_arena_contains(arena_id: mi_arena_id_t, p: *const c_void) -> bool;

   /// Creates a heap that only allocates from `arena_id`.
   pub fn mi_heap_new_in_arena(arena_id: mi_arena_id_t) -> *mut mi_heap_t;

   /// Returns the main subprocess.
   pub fn mi_subproc_main() -> mi_subproc_id_t;

   /// Returns the subprocess of the calling thread.
   pub fn mi_subproc_current() -> mi_subproc_id_t;

   /// Creates a subprocess.
   pub fn mi_subproc_new() -> mi_subproc_id_t;

   /// Destroys `subproc`.
   pub fn mi_subproc_destroy(subproc: mi_subproc_id_t);

   /// Moves the calling thread into `subproc`. Call before the thread
   /// allocates.
   pub fn mi_subproc_add_current_thread(subproc: mi_subproc_id_t);

   /// Visits every heap of `subproc`. Returns `true` if the visit completed.
   pub fn mi_subproc_visit_heaps(
      subproc: mi_subproc_id_t,
      visitor: Option<mi_heap_visit_fun>,
      arg: *mut c_void,
   ) -> bool;

   /// Returns the calling thread's theap for `heap`.
   pub fn mi_heap_theap(heap: *mut mi_heap_t) -> *mut mi_theap_t;

   /// Makes `theap` the target of [`mi_malloc`] on the calling thread. Returns
   /// the previous one.
   pub fn mi_theap_set_default(theap: *mut mi_theap_t) -> *mut mi_theap_t;

   /// Returns the default theap of the calling thread.
   pub fn mi_theap_get_default() -> *mut mi_theap_t;

   /// Returns free memory of `theap` to the OS.
   pub fn mi_theap_collect(theap: *mut mi_theap_t, force: bool);

   /// [`mi_malloc`] from `theap`.
   pub fn mi_theap_malloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void;

   /// [`mi_zalloc`] from `theap`.
   pub fn mi_theap_zalloc(theap: *mut mi_theap_t, size: usize) -> *mut c_void;

   /// [`mi_calloc`] from `theap`.
   pub fn mi_theap_calloc(theap: *mut mi_theap_t, count: usize, size: usize) -> *mut c_void;

   /// [`mi_malloc_small`] from `theap`.
   pub fn mi_theap_malloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void;

   /// [`mi_zalloc_small`] from `theap`.
   pub fn mi_theap_zalloc_small(theap: *mut mi_theap_t, size: usize) -> *mut c_void;

   /// [`mi_malloc_aligned`] from `theap`.
   pub fn mi_theap_malloc_aligned(
      theap: *mut mi_theap_t,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_zalloc_aligned`] from `theap`.
   pub fn mi_theap_zalloc_aligned(
      theap: *mut mi_theap_t,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// [`mi_realloc`] within `theap`.
   pub fn mi_theap_realloc(theap: *mut mi_theap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

   /// [`mi_rezalloc`] within `theap`.
   pub fn mi_theap_rezalloc(theap: *mut mi_theap_t, p: *mut c_void, newsize: usize) -> *mut c_void;

   /// Guards about one object per `sample_rate` bytes on `theap`, 0 disabling
   /// it. A `seed` of 0 picks a random start. Experimental.
   pub fn mi_theap_guarded_set_sample_rate(theap: *mut mi_theap_t, sample_rate: usize, seed: usize);

   /// Restricts guarded objects on `theap` to sizes in `min..=max`.
   /// Experimental.
   pub fn mi_theap_guarded_set_size_bound(theap: *mut mi_theap_t, min: usize, max: usize);

   /// [`mi_manage_os_memory_ex`] with a callback that commits and decommits the
   /// memory. Experimental.
   pub fn mi_manage_memory(
      start: *mut c_void,
      size: usize,
      is_committed: bool,
      is_pinned: bool,
      is_zero: bool,
      numa_node: c_int,
      exclusive: bool,
      commit_fun: Option<mi_commit_fun_t>,
      commit_fun_arg: *mut c_void,
      arena_id: *mut mi_arena_id_t,
   ) -> bool;

   /// Returns whether the page holding `p` is used below `perc_threshold`
   /// percent. The page must only be accessed by the calling thread.
   pub fn mi_unsafe_heap_page_is_under_utilized(
      heap: *mut mi_heap_t,
      p: *mut c_void,
      perc_threshold: usize,
   ) -> bool;

   /// Abandons the pages of `theap` and deletes it, keeping its heap and live
   /// blocks. Owner thread only.
   pub fn mi_theap_delete(theap: *mut mi_theap_t);

   /// Abandons the pages of `theap`, keeping its live blocks and the theap
   /// itself. The owner must be the calling thread or be outside mimalloc.
   pub fn mi_theap_quiescent_abandon(theap: *mut mi_theap_t);

   /// Returns whether `option` is enabled.
   pub fn mi_option_is_enabled(option: mi_option_t) -> bool;

   /// Enables `option`.
   pub fn mi_option_enable(option: mi_option_t);

   /// Disables `option`.
   pub fn mi_option_disable(option: mi_option_t);

   /// Enables or disables `option`.
   pub fn mi_option_set_enabled(option: mi_option_t, enable: bool);

   /// Enables or disables `option` unless it was already set.
   pub fn mi_option_set_enabled_default(option: mi_option_t, enable: bool);

   /// Returns the value of `option`.
   pub fn mi_option_get(option: mi_option_t) -> c_long;

   /// Returns the value of `option` clamped to `min..=max`.
   pub fn mi_option_get_clamp(option: mi_option_t, min: c_long, max: c_long) -> c_long;

   /// Returns the value of a size option in bytes.
   pub fn mi_option_get_size(option: mi_option_t) -> usize;

   /// Sets `option` to `value`.
   pub fn mi_option_set(option: mi_option_t, value: c_long);

   /// Sets `option` to `value` unless it was already set.
   pub fn mi_option_set_default(option: mi_option_t, value: c_long);

   /// Frees `p` if it belongs to mimalloc. Returns whether it did.
   pub fn mi_cfree(p: *mut c_void) -> bool;

   /// Windows `_expand`.
   pub fn mi__expand(p: *mut c_void, newsize: usize) -> *mut c_void;

   /// macOS `malloc_size`.
   pub fn mi_malloc_size(p: *const c_void) -> usize;

   /// macOS `malloc_good_size`.
   pub fn mi_malloc_good_size(size: usize) -> usize;

   /// Linux `malloc_usable_size`.
   pub fn mi_malloc_usable_size(p: *const c_void) -> usize;

   /// POSIX `posix_memalign`. Returns 0, `EINVAL`, or `ENOMEM`.
   pub fn mi_posix_memalign(p: *mut *mut c_void, alignment: usize, size: usize) -> c_int;

   /// `memalign`.
   pub fn mi_memalign(alignment: usize, size: usize) -> *mut c_void;

   /// `valloc`.
   pub fn mi_valloc(size: usize) -> *mut c_void;

   /// `pvalloc`.
   pub fn mi_pvalloc(size: usize) -> *mut c_void;

   /// C11 `aligned_alloc`.
   pub fn mi_aligned_alloc(alignment: usize, size: usize) -> *mut c_void;

   /// BSD `reallocarray`.
   pub fn mi_reallocarray(p: *mut c_void, count: usize, size: usize) -> *mut c_void;

   /// NetBSD `reallocarr`. `ptrp` points to the pointer to resize.
   pub fn mi_reallocarr(ptrp: *mut c_void, count: usize, size: usize) -> c_int;

   /// Windows `_aligned_recalloc`.
   pub fn mi_aligned_recalloc(
      p: *mut c_void,
      newcount: usize,
      size: usize,
      alignment: usize,
   ) -> *mut c_void;

   /// Windows `_aligned_offset_recalloc`.
   pub fn mi_aligned_offset_recalloc(
      p: *mut c_void,
      newcount: usize,
      size: usize,
      alignment: usize,
      offset: usize,
   ) -> *mut c_void;

   /// C++ sized and aligned `operator delete`.
   pub fn mi_free_size_aligned(p: *mut c_void, size: usize, alignment: usize);

   /// C++ aligned `operator delete`.
   pub fn mi_free_aligned(p: *mut c_void, alignment: usize);

   /// Fills `stats` with the statistics of `heap`. Returns `false` on a size or
   /// version mismatch.
   pub fn mi_heap_stats_get(heap: *mut mi_heap_t, stats: *mut mi_stats_t) -> bool;

   /// Statistics of `heap` as JSON. With a null `buf` the result is allocated
   /// and must be freed with [`mi_free`]. Returns null on failure.
   pub fn mi_heap_stats_get_json(
      heap: *mut mi_heap_t,
      buf_size: usize,
      buf: *mut c_char,
   ) -> *mut c_char;

   /// Prints the statistics of `heap` through `out`.
   pub fn mi_heap_stats_print_out(
      heap: *mut mi_heap_t,
      out: Option<mi_output_fun>,
      arg: *mut c_void,
   );

   /// Fills `stats` with the statistics of `subproc` and its heaps.
   pub fn mi_subproc_stats_get(subproc: mi_subproc_id_t, stats: *mut mi_stats_t) -> bool;

   /// Statistics of `subproc` and its heaps as JSON, see
   /// [`mi_heap_stats_get_json`].
   pub fn mi_subproc_stats_get_json(
      subproc: mi_subproc_id_t,
      buf_size: usize,
      buf: *mut c_char,
   ) -> *mut c_char;

   /// Prints the statistics of `subproc` and its heaps, aggregated, through
   /// `out`.
   pub fn mi_subproc_stats_print_out(
      subproc: mi_subproc_id_t,
      out: Option<mi_output_fun>,
      arg: *mut c_void,
   );

   /// Prints the statistics of `subproc` and of each of its heaps through
   /// `out`.
   pub fn mi_subproc_heap_stats_print_out(
      subproc: mi_subproc_id_t,
      out: Option<mi_output_fun>,
      arg: *mut c_void,
   );

   /// Fills `stats` with the statistics of the current subprocess and its
   /// heaps.
   pub fn mi_stats_get(stats: *mut mi_stats_t) -> bool;

   /// Statistics of the current subprocess as JSON, see
   /// [`mi_heap_stats_get_json`].
   pub fn mi_stats_get_json(buf_size: usize, buf: *mut c_char) -> *mut c_char;

   /// Prints the statistics of the current subprocess through `out`, or stderr
   /// with `None`.
   pub fn mi_stats_print_out(out: Option<mi_output_fun>, arg: *mut c_void);

   /// Adds the statistics of `heap` to its subprocess and clears them.
   pub fn mi_heap_stats_merge_to_subproc(heap: *mut mi_heap_t);

   /// Fills `stats` with the statistics of `subproc` alone, without its heaps.
   pub fn mi_subproc_stats_get_exclusive(subproc: mi_subproc_id_t, stats: *mut mi_stats_t) -> bool;

   /// Formats `stats` as JSON, see [`mi_heap_stats_get_json`].
   pub fn mi_stats_as_json(
      stats: *mut mi_stats_t,
      buf_size: usize,
      buf: *mut c_char,
   ) -> *mut c_char;

   /// Returns the block size of size bin `bin`.
   pub fn mi_stats_get_bin_size(bin: usize) -> usize;

   /// Fills `stats` with the statistics of `theap`. Only thread safe on the
   /// owning thread.
   pub fn mi_theap_stats_get(theap: *mut mi_theap_t, stats: *mut mi_stats_t) -> bool;

   /// Adds the statistics of `theap` to its heap and clears them.
   pub fn mi_theap_stats_merge_to_heap(theap: *mut mi_theap_t);
}

/// The C side is one object file, so referencing `mi_malloc` keeps the whole
/// archive member, its malloc overrides, and the macOS zone constructor alive
/// even when nothing else calls into it (<https://maskray.me/blog/2021-06-20-symbol-processing#archive-processing>).
#[cfg(feature = "override")]
#[used]
static KEEP_OVERRIDES: unsafe extern "C" fn(usize) -> *mut c_void = mi_malloc;
