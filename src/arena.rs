use core::{
   error::Error,
   ffi::{
      c_int,
      c_void,
   },
   fmt,
   num::NonZeroUsize,
   ptr,
};

use mimalloc_ng_sys::{
   mi_arena_contains,
   mi_arena_id_t,
   mi_reserve_os_memory_ex,
};

/// An arena of OS memory reserved for mimalloc. Arenas live until the process
/// exits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Arena(mi_arena_id_t);

// SAFETY: the arena id is an opaque handle that mimalloc accepts from any
// thread.
unsafe impl Send for Arena {}

// SAFETY: as above, the handle is never written through.
unsafe impl Sync for Arena {}

/// How [`Arena::reserve`] sets up the memory.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ArenaConfig {
   /// Commits the memory up front.
   pub commit:      bool,
   /// Allows large (2 or 4 MiB) OS pages.
   pub allow_large: bool,
   /// Serves only heaps created with
   /// [`Heap::in_arena`](crate::heap::Heap::in_arena).
   pub exclusive:   bool,
}

impl Arena {
   /// Reserves `size` bytes of OS memory as an arena.
   ///
   /// # Errors
   ///
   /// Returns the `errno` code mimalloc reports, such as `ENOMEM`.
   #[inline]
   pub fn reserve(size: NonZeroUsize, config: ArenaConfig) -> Result<Self, ReserveError> {
      let mut id: mi_arena_id_t = ptr::null_mut();

      // SAFETY: `id` is a valid out-pointer.
      let errno = unsafe {
         mi_reserve_os_memory_ex(
            size.get(),
            config.commit,
            config.allow_large,
            config.exclusive,
            &raw mut id,
         )
      };

      if errno != 0_i32 {
         return Err(ReserveError(errno));
      }

      Ok(Self(id))
   }

   /// Returns whether `ptr` lies inside this arena. Any pointer is valid input.
   #[inline]
   #[must_use = "this only queries the arena"]
   pub fn contains<T>(self, ptr: *const T) -> bool
   where
      T: ?Sized,
   {
      // SAFETY: mimalloc only compares `ptr` against the arena bounds.
      unsafe { mi_arena_contains(self.0, ptr.cast::<c_void>()) }
   }

   /// Returns the raw arena id for use with `mimalloc_ng_sys`.
   #[inline]
   #[must_use = "this only returns the id"]
   pub const fn id(self) -> mi_arena_id_t {
      self.0
   }
}

/// mimalloc could not reserve an arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReserveError(c_int);

impl ReserveError {
   /// The `errno` code mimalloc returned.
   #[inline]
   #[must_use = "this only reads the code"]
   pub const fn errno(self) -> c_int {
      self.0
   }
}

impl fmt::Display for ReserveError {
   #[inline]
   #[expect(
      clippy::renamed_function_params,
      reason = "the trait's `f` is shorter than the two-character minimum"
   )]
   fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
      write!(
         formatter,
         "mimalloc could not reserve an arena (errno {})",
         self.0
      )
   }
}

impl Error for ReserveError {}
