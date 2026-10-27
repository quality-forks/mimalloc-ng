use core::{
   mem::ManuallyDrop,
   ptr::{
      self,
      NonNull,
   },
};

use mimalloc_ng_sys::{
   mi_heap_collect,
   mi_heap_contains,
   mi_heap_delete,
   mi_heap_destroy,
   mi_heap_new,
   mi_heap_new_in_arena,
   mi_heap_stats_get,
   mi_heap_stats_get_json,
   mi_heap_t,
   mi_stats_t,
};

use crate::{
   StatsJson,
   arena::Arena,
};

/// A first-class mimalloc heap.
///
/// Its blocks live in their own pages, so related allocations stay together and
/// can be released in one go. Any thread may allocate from a heap, and any
/// thread may free its blocks. Dropping the heap moves its live blocks to the
/// main heap, so they stay valid.
#[derive(Debug)]
pub struct Heap(NonNull<mi_heap_t>);

// SAFETY: mimalloc heaps are first-class, every thread may allocate from them
// and free their blocks (`mimalloc.h`, "Heaps").
unsafe impl Send for Heap {}

// SAFETY: as above, allocation through a shared heap is thread safe.
unsafe impl Sync for Heap {}

impl Heap {
   /// Creates a heap, or returns `None` if mimalloc is out of memory.
   #[inline]
   #[must_use = "dropping the heap deletes it"]
   pub fn new() -> Option<Self> {
      // SAFETY: takes no input.
      NonNull::new(unsafe { mi_heap_new() }).map(Self)
   }

   /// Creates a heap that allocates only from `arena`.
   #[inline]
   #[must_use = "dropping the heap deletes it"]
   pub fn in_arena(arena: Arena) -> Option<Self> {
      // SAFETY: `arena` came from a successful reservation and arenas are
      // never released.
      NonNull::new(unsafe { mi_heap_new_in_arena(arena.id()) }).map(Self)
   }

   /// Returns whether `ptr` points into a block of this heap. Any pointer is
   /// valid input.
   #[inline]
   #[must_use = "this only queries the heap"]
   pub fn contains<T>(&self, ptr: *const T) -> bool
   where
      T: ?Sized,
   {
      // SAFETY: mimalloc looks `ptr` up in its page map, which accepts any
      // address.
      unsafe { mi_heap_contains(self.0.as_ptr(), ptr.cast()) }
   }

   /// Returns free memory held for this heap by the calling thread to the OS.
   #[inline]
   pub fn collect(&self, force: bool) {
      // SAFETY: the heap is live.
      unsafe { mi_heap_collect(self.0.as_ptr(), force) }
   }

   /// Returns the statistics of this heap, or `None` if mimalloc rejects the
   /// size or version of `mi_stats_t`.
   ///
   /// Allocation counters lag as described in [`stats`](crate::stats).
   #[inline]
   #[must_use = "this only reads the statistics"]
   pub fn stats(&self) -> Option<mi_stats_t> {
      let mut stats = mi_stats_t::new();

      // SAFETY: the heap is live and `stats` carries its own size and version.
      unsafe { mi_heap_stats_get(self.0.as_ptr(), &raw mut stats) }.then_some(stats)
   }

   /// Returns the statistics of this heap as JSON, or `None` if mimalloc is out
   /// of memory.
   #[inline]
   #[must_use = "the string is freed when dropped"]
   pub fn stats_json(&self) -> Option<StatsJson> {
      // SAFETY: the heap is live and a null buffer makes mimalloc allocate the
      // result itself.
      let json = unsafe { mi_heap_stats_get_json(self.0.as_ptr(), 0, ptr::null_mut()) };

      // SAFETY: mimalloc returns null or a string allocated with `mi_malloc`.
      unsafe { StatsJson::from_raw(json) }
   }

   /// Frees every block of this heap at once, without moving them to the main
   /// heap.
   ///
   /// # Safety
   ///
   /// No block allocated from this heap may be used or freed afterwards.
   #[inline]
   pub unsafe fn destroy(self) {
      let heap = ManuallyDrop::new(self);

      // SAFETY: the caller guarantees no block of the heap outlives it, and
      // `ManuallyDrop` keeps `drop` from deleting it again.
      unsafe { mi_heap_destroy(heap.0.as_ptr()) }
   }

   /// Returns the raw heap for use with `mimalloc_ng_sys`.
   #[inline]
   #[must_use = "this only returns the pointer"]
   pub const fn as_ptr(&self) -> *mut mi_heap_t {
      self.0.as_ptr()
   }
}

impl Drop for Heap {
   #[inline]
   fn drop(&mut self) {
      // SAFETY: the heap is live and is deleted exactly once.
      unsafe { mi_heap_delete(self.0.as_ptr()) }
   }
}
