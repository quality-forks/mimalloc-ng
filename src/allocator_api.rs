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
   mi_heap_malloc_aligned,
   mi_heap_realloc_aligned,
   mi_heap_zalloc_aligned,
   mi_malloc_aligned,
   mi_realloc_aligned,
   mi_usable_size,
   mi_zalloc_aligned,
};

use crate::{
   MiMalloc,
   heap::Heap,
};

/// The mimalloc entry points an [`Allocator`] needs, for the default heap or a
/// [`Heap`].
trait Source {
   /// Allocates a non-empty `layout`.
   fn malloc(&self, layout: Layout) -> *mut c_void;

   /// Allocates a non-empty, zeroed `layout`.
   fn zalloc(&self, layout: Layout) -> *mut c_void;

   /// Resizes the block at `ptr` to the non-empty `layout`.
   ///
   /// # Safety
   ///
   /// `ptr` must be a live mimalloc block.
   unsafe fn realloc(&self, ptr: NonNull<u8>, layout: Layout) -> *mut c_void;
}

impl Source for MiMalloc {
   fn malloc(&self, layout: Layout) -> *mut c_void {
      // SAFETY: any size and power-of-two alignment is valid input.
      unsafe { mi_malloc_aligned(layout.size(), layout.align()) }
   }

   fn zalloc(&self, layout: Layout) -> *mut c_void {
      // SAFETY: any size and power-of-two alignment is valid input.
      unsafe { mi_zalloc_aligned(layout.size(), layout.align()) }
   }

   unsafe fn realloc(&self, ptr: NonNull<u8>, layout: Layout) -> *mut c_void {
      // SAFETY: the caller guarantees `ptr` is a live mimalloc block.
      unsafe { mi_realloc_aligned(ptr.as_ptr().cast(), layout.size(), layout.align()) }
   }
}

impl Source for Heap {
   fn malloc(&self, layout: Layout) -> *mut c_void {
      // SAFETY: the heap is live and any power-of-two alignment is valid.
      unsafe { mi_heap_malloc_aligned(self.as_ptr(), layout.size(), layout.align()) }
   }

   fn zalloc(&self, layout: Layout) -> *mut c_void {
      // SAFETY: the heap is live and any power-of-two alignment is valid.
      unsafe { mi_heap_zalloc_aligned(self.as_ptr(), layout.size(), layout.align()) }
   }

   unsafe fn realloc(&self, ptr: NonNull<u8>, layout: Layout) -> *mut c_void {
      let (size, align) = (layout.size(), layout.align());

      // SAFETY: the heap is live and the caller guarantees `ptr` is a live
      // mimalloc block.
      unsafe { mi_heap_realloc_aligned(self.as_ptr(), ptr.as_ptr().cast(), size, align) }
   }
}

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

/// [`Allocator::allocate`] for any [`Source`].
fn allocate(source: &impl Source, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
   if layout.size() == 0 {
      return Ok(empty(layout));
   }

   // SAFETY: mimalloc returns null or a fresh block.
   unsafe { block(source.malloc(layout)) }
}

/// [`Allocator::allocate_zeroed`] for any [`Source`].
fn allocate_zeroed(source: &impl Source, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
   if layout.size() == 0 {
      return Ok(empty(layout));
   }

   // SAFETY: mimalloc returns null or a fresh block.
   unsafe { block(source.zalloc(layout)) }
}

/// [`Allocator::deallocate`] for any [`Source`].
///
/// # Safety
///
/// `ptr` must have been allocated by a [`Source`] with `layout`.
unsafe fn deallocate(ptr: NonNull<u8>, layout: Layout) {
   if layout.size() != 0 {
      // SAFETY: a non-empty `layout` means `ptr` is a mimalloc block.
      unsafe { mi_free(ptr.as_ptr().cast()) }
   }
}

/// [`Allocator::grow`] for any [`Source`].
///
/// # Safety
///
/// Same contract as [`Allocator::grow`].
unsafe fn grow(
   source: &impl Source,
   ptr: NonNull<u8>,
   old_layout: Layout,
   new_layout: Layout,
) -> Result<NonNull<[u8]>, AllocError> {
   if old_layout.size() == 0 {
      return allocate(source, new_layout);
   }

   // SAFETY: a non-empty `old_layout` means `ptr` is a mimalloc block.
   let raw = unsafe { source.realloc(ptr, new_layout) };

   // SAFETY: mimalloc returns null or a live block.
   unsafe { block(raw) }
}

/// [`Allocator::grow_zeroed`] for any [`Source`].
///
/// # Safety
///
/// Same contract as [`Allocator::grow_zeroed`].
unsafe fn grow_zeroed(
   source: &impl Source,
   ptr: NonNull<u8>,
   old_layout: Layout,
   new_layout: Layout,
) -> Result<NonNull<[u8]>, AllocError> {
   // SAFETY: `grow` has the same contract as this function.
   let grown = unsafe { grow(source, ptr, old_layout, new_layout) }?;
   let tail = grown.len() - old_layout.size();

   // SAFETY: `old_layout.size()..grown.len()` lies inside the grown block.
   let start = unsafe { grown.cast::<u8>().add(old_layout.size()) };

   // SAFETY: `start` is valid for `tail` bytes of writes.
   unsafe { start.write_bytes(0, tail) }
   Ok(grown)
}

/// [`Allocator::shrink`] for any [`Source`].
///
/// # Safety
///
/// Same contract as [`Allocator::shrink`].
unsafe fn shrink(
   source: &impl Source,
   ptr: NonNull<u8>,
   old_layout: Layout,
   new_layout: Layout,
) -> Result<NonNull<[u8]>, AllocError> {
   if new_layout.size() == 0 {
      // SAFETY: the caller guarantees `ptr` was allocated with `old_layout`.
      unsafe { deallocate(ptr, old_layout) }
      return Ok(empty(new_layout));
   }

   // SAFETY: a non-empty `new_layout` means `old_layout` is non-empty too, so
   // `ptr` is a mimalloc block.
   let raw = unsafe { source.realloc(ptr, new_layout) };

   // SAFETY: mimalloc returns null or a live block.
   unsafe { block(raw) }
}

/// Implements [`Allocator`] by forwarding to the functions above.
macro_rules! impl_allocator {
   ($ty:ty) => {
      // SAFETY: blocks stay valid until freed, even past the allocator, since
      // mimalloc moves the live blocks of a deleted heap to the main heap.
      // Zero-size layouts never reach mimalloc.
      unsafe impl Allocator for $ty {
         #[inline]
         fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            allocate(self, layout)
         }

         #[inline]
         fn allocate_zeroed(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
            allocate_zeroed(self, layout)
         }

         #[inline]
         unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
            // SAFETY: the caller upholds the same contract.
            unsafe { deallocate(ptr, layout) }
         }

         #[inline]
         unsafe fn grow(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
         ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: the caller upholds the same contract.
            unsafe { grow(self, ptr, old_layout, new_layout) }
         }

         #[inline]
         unsafe fn grow_zeroed(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
         ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: the caller upholds the same contract.
            unsafe { grow_zeroed(self, ptr, old_layout, new_layout) }
         }

         #[inline]
         unsafe fn shrink(
            &self,
            ptr: NonNull<u8>,
            old_layout: Layout,
            new_layout: Layout,
         ) -> Result<NonNull<[u8]>, AllocError> {
            // SAFETY: the caller upholds the same contract.
            unsafe { shrink(self, ptr, old_layout, new_layout) }
         }
      }
   };
}

impl_allocator!(MiMalloc);
impl_allocator!(Heap);
