use core::{
   alloc::{
      AllocError,
      Allocator,
      Layout,
   },
   ffi::c_void,
   ptr::NonNull,
};

use mimalloc_ng_sys::{
   mi_free,
   mi_malloc_aligned,
   mi_realloc_aligned,
   mi_usable_size,
   mi_zalloc_aligned,
};

use crate::MiMalloc;

/// The dangling, zero-length block handed out for zero-size layouts.
const fn empty(layout: Layout) -> NonNull<[u8]> {
   NonNull::slice_from_raw_parts(layout.dangling_ptr(), 0)
}

/// Wraps a mimalloc result as a slice spanning the whole usable block.
///
/// # Safety
///
/// `raw` must be null or a live block allocated by mimalloc.
unsafe fn block(raw: *mut c_void) -> Result<NonNull<[u8]>, AllocError> {
   let ptr = NonNull::new(raw.cast::<u8>()).ok_or(AllocError)?;

   // SAFETY: `raw` is a live mimalloc block.
   let len = unsafe { mi_usable_size(raw) };
   Ok(NonNull::slice_from_raw_parts(ptr, len))
}

/// Resizes the mimalloc block at `ptr` to `layout`.
///
/// # Safety
///
/// `ptr` must be a live block allocated by mimalloc.
unsafe fn resize(ptr: NonNull<u8>, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
   // SAFETY: the caller guarantees `ptr` is a live mimalloc block.
   let raw = unsafe { mi_realloc_aligned(ptr.as_ptr().cast(), layout.size(), layout.align()) };

   // SAFETY: mimalloc returns null or a live block.
   unsafe { block(raw) }
}

// SAFETY: blocks stay valid until freed and are not tied to the `MiMalloc`
// value, and zero-size layouts never reach mimalloc.
unsafe impl Allocator for MiMalloc {
   #[inline]
   fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
      if layout.size() == 0 {
         return Ok(empty(layout));
      }

      // SAFETY: any size and power-of-two alignment is valid input.
      let raw = unsafe { mi_malloc_aligned(layout.size(), layout.align()) };

      // SAFETY: mimalloc returns null or a fresh block.
      unsafe { block(raw) }
   }

   #[inline]
   fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
      if layout.size() == 0 {
         return Ok(empty(layout));
      }

      // SAFETY: any size and power-of-two alignment is valid input.
      let raw = unsafe { mi_zalloc_aligned(layout.size(), layout.align()) };

      // SAFETY: mimalloc returns null or a fresh block.
      unsafe { block(raw) }
   }

   #[inline]
   unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
      if layout.size() != 0 {
         // SAFETY: a non-empty `layout` means `ptr` is a mimalloc block.
         unsafe { mi_free(ptr.as_ptr().cast()) }
      }
   }

   #[inline]
   unsafe fn grow(
      &self,
      ptr: NonNull<u8>,
      old_layout: Layout,
      new_layout: Layout,
   ) -> Result<NonNull<[u8]>, AllocError> {
      if old_layout.size() == 0 {
         return self.allocate(new_layout);
      }

      // SAFETY: a non-empty `old_layout` means `ptr` is a mimalloc block.
      unsafe { resize(ptr, new_layout) }
   }

   #[inline]
   unsafe fn grow_zeroed(
      &self,
      ptr: NonNull<u8>,
      old_layout: Layout,
      new_layout: Layout,
   ) -> Result<NonNull<[u8]>, AllocError> {
      // SAFETY: `grow` has the same contract as this method.
      let grown = unsafe { self.grow(ptr, old_layout, new_layout) }?;
      let tail = grown.len() - old_layout.size();

      // SAFETY: `old_layout.size()..grown.len()` lies inside the grown block.
      let start = unsafe { grown.cast::<u8>().add(old_layout.size()) };

      // SAFETY: `start` is valid for `tail` bytes of writes.
      unsafe { start.write_bytes(0, tail) }
      Ok(grown)
   }

   #[inline]
   unsafe fn shrink(
      &self,
      ptr: NonNull<u8>,
      old_layout: Layout,
      new_layout: Layout,
   ) -> Result<NonNull<[u8]>, AllocError> {
      if new_layout.size() == 0 {
         // SAFETY: the caller guarantees `ptr` was allocated with `old_layout`.
         unsafe { self.deallocate(ptr, old_layout) }
         return Ok(empty(new_layout));
      }

      // SAFETY: a non-empty `new_layout` means `old_layout` is non-empty too,
      // so `ptr` is a mimalloc block.
      unsafe { resize(ptr, new_layout) }
   }
}
