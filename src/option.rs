//! mimalloc runtime options, split by the type of their value. Every option
//! can also be set through its `MIMALLOC_<NAME>` environment variable before
//! the first allocation.
//!
//! mimalloc writes options without synchronization, so every setter is unsafe.
//! No other thread may run mimalloc code during the call, so set options before
//! spawning threads.

use core::{
   ffi::c_long,
   num::TryFromIntError,
};

use mimalloc_ng_sys::{
   mi_option_allow_large_os_pages,
   mi_option_allow_thp,
   mi_option_arena_eager_commit,
   mi_option_arena_is_numa_local,
   mi_option_arena_max_object_size,
   mi_option_arena_purge_immediate_size,
   mi_option_arena_purge_min_size,
   mi_option_arena_purge_mult,
   mi_option_arena_reserve,
   mi_option_destroy_on_exit,
   mi_option_disallow_arena_alloc,
   mi_option_disallow_os_alloc,
   mi_option_generic_collect,
   mi_option_get,
   mi_option_get_size,
   mi_option_guarded_max,
   mi_option_guarded_min,
   mi_option_guarded_sample_rate,
   mi_option_guarded_sample_seed,
   mi_option_is_enabled,
   mi_option_max_errors,
   mi_option_max_vabits,
   mi_option_max_warnings,
   mi_option_os_tag,
   mi_option_page_commit_on_demand,
   mi_option_page_cross_thread_max_reclaim,
   mi_option_page_full_retain,
   mi_option_page_max_candidates,
   mi_option_page_max_reclaim,
   mi_option_page_reclaim_on_free,
   mi_option_pagemap_commit,
   mi_option_profile_alloc_interval,
   mi_option_profile_inuse_interval,
   mi_option_profile_sample_rate,
   mi_option_profile_time_interval,
   mi_option_purge_decommits,
   mi_option_purge_delay,
   mi_option_reserve_huge_os_pages,
   mi_option_reserve_huge_os_pages_at,
   mi_option_reserve_os_memory,
   mi_option_retry_on_oom,
   mi_option_set,
   mi_option_set_default,
   mi_option_set_enabled,
   mi_option_set_enabled_default,
   mi_option_show_errors,
   mi_option_show_stats,
   mi_option_stats_merge_on_collect,
   mi_option_stats_merge_threshold,
   mi_option_t,
   mi_option_unsafe_guarded_precise,
   mi_option_use_numa_nodes,
   mi_option_verbose,
};

/// An on or off option.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Flag {
   /// Print error messages.
   ShowErrors,
   /// Print statistics on termination.
   ShowStats,
   /// Decommit on purge (=on). Off resets memory instead.
   PurgeDecommits,
   /// Allow large (2 or 4 MiB) OS pages, implies eager commit.
   AllowLargeOsPages,
   /// Allocate only from reserved arenas, never directly from the OS.
   DisallowOsAlloc,
   /// Release all memory on exit, which breaks frees that run after exit.
   DestroyOnExit,
   /// Allocate only from explicitly requested arenas.
   DisallowArenaAlloc,
   /// Place guarded blocks right before the guard page, ignoring the minimal
   /// alignment.
   UnsafeGuardedPrecise,
   /// Commit the full page map to always catch invalid pointers.
   PagemapCommit,
   /// Associate the local NUMA node with the initial arena. Experimental.
   ArenaIsNumaLocal,
   /// Merge theap statistics into the parent heap on every collection.
   StatsMergeOnCollect,
}

/// A size option, in bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Size {
   /// OS memory to reserve in an arena at startup.
   ReserveOsMemory,
   /// Initial arena reservation (=1 GiB on 64-bit).
   ArenaReserve,
   /// Minimal purge size (=0, meaning 64 KiB, or 2 MiB with THP).
   ArenaPurgeMinSize,
   /// Maximal object size allocated in an arena (=2 GiB on 64-bit).
   ArenaMaxObjectSize,
   /// Purge ranges above this size immediately.
   ArenaPurgeImmediateSize,
   /// Minimal rounded object size for guarded objects (=0).
   GuardedMin,
   /// Maximal rounded object size for guarded objects (=1 GiB).
   GuardedMax,
   /// Guard about one object per this many requested bytes (=0).
   GuardedSampleRate,
   /// Allocated bytes between automatic heap profiler snapshots (=1 GiB).
   ProfileAllocInterval,
   /// In-use growth between automatic heap profiler snapshots (=0, disabled).
   ProfileInuseInterval,
   /// Heap profiler sample rate (=512 KiB).
   ProfileSampleRate,
   /// Merge theap statistics into the parent heap after this many bytes change
   /// (=512 KiB, 0 disables).
   StatsMergeThreshold,
}

/// A numeric option: a count, a duration, or a level.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Number {
   /// Verbosity level, 2 and up adds more detail.
   Verbose,
   /// Eagerly commit arenas. 2 does so only on overcommit systems (=2).
   ArenaEagerCommit,
   /// Reserve N huge (1 GiB) OS pages at startup.
   ReserveHugeOsPages,
   /// Reserve the huge OS pages at NUMA node N (=-1, any).
   ReserveHugeOsPagesAt,
   /// Delay purging by N milliseconds. 0 purges immediately, -1 never (=1000).
   PurgeDelay,
   /// Use at most N NUMA nodes. 0 uses all.
   UseNumaNodes,
   /// Tag for OS memory accounting, macOS only (=100).
   OsTag,
   /// Print at most N error messages (=32).
   MaxErrors,
   /// Print at most N warning messages (=32).
   MaxWarnings,
   /// Multiplier of `PurgeDelay` for arenas (=4).
   ArenaPurgeMult,
   /// Retry for N milliseconds on out-of-memory, Windows only (=400).
   RetryOnOom,
   /// Seed for guarded sampling, for reproducible runs (=0).
   GuardedSampleSeed,
   /// Collect theaps every N generic allocation calls (=10000).
   GenericCollect,
   /// Reclaim abandoned pages on free. -1 never, 0 into the originating theap,
   /// 1 into any theap (=1).
   PageReclaimOnFree,
   /// Retain N full small pages per size class (=2).
   PageFullRetain,
   /// Consider at most N candidate pages per allocation (=4).
   PageMaxCandidates,
   /// Maximum user-space virtual address bits, 0 detects them (=0).
   MaxVabits,
   /// Commit page memory on demand. 2 does so only on overcommit systems.
   PageCommitOnDemand,
   /// Stop reclaiming pages of the originating theap once N are owned per size
   /// class (=-1, unlimited).
   PageMaxReclaim,
   /// Stop reclaiming cross-thread pages once N are owned per size class (=32).
   PageCrossThreadMaxReclaim,
   /// Allow transparent huge pages (=1, Android =0). 2 also keeps purges from
   /// splitting them.
   AllowThp,
   /// Seconds between automatic heap profiler snapshots (=0, disabled).
   ProfileTimeInterval,
}

impl Flag {
   /// The `mi_option_t` value for this option.
   const fn raw(self) -> mi_option_t {
      match self {
         Self::ShowErrors => mi_option_show_errors,
         Self::ShowStats => mi_option_show_stats,
         Self::PurgeDecommits => mi_option_purge_decommits,
         Self::AllowLargeOsPages => mi_option_allow_large_os_pages,
         Self::DisallowOsAlloc => mi_option_disallow_os_alloc,
         Self::DestroyOnExit => mi_option_destroy_on_exit,
         Self::DisallowArenaAlloc => mi_option_disallow_arena_alloc,
         Self::UnsafeGuardedPrecise => mi_option_unsafe_guarded_precise,
         Self::PagemapCommit => mi_option_pagemap_commit,
         Self::ArenaIsNumaLocal => mi_option_arena_is_numa_local,
         Self::StatsMergeOnCollect => mi_option_stats_merge_on_collect,
      }
   }

   /// Returns whether the option is on.
   #[inline]
   #[must_use = "this only reads the option"]
   pub fn is_enabled(self) -> bool {
      // SAFETY: `raw` is a valid option.
      unsafe { mi_option_is_enabled(self.raw()) }
   }

   /// Turns the option on or off.
   ///
   /// # Safety
   ///
   /// No other thread may run mimalloc code during the call.
   #[inline]
   pub unsafe fn set(self, enabled: bool) {
      // SAFETY: the caller guarantees no concurrent mimalloc calls.
      unsafe { mi_option_set_enabled(self.raw(), enabled) }
   }

   /// Turns the option on or off unless code or the environment already set it.
   ///
   /// # Safety
   ///
   /// No other thread may run mimalloc code during the call.
   #[inline]
   pub unsafe fn set_default(self, enabled: bool) {
      // SAFETY: the caller guarantees no concurrent mimalloc calls.
      unsafe { mi_option_set_enabled_default(self.raw(), enabled) }
   }
}

impl Size {
   /// The `mi_option_t` value for this option.
   const fn raw(self) -> mi_option_t {
      match self {
         Self::ReserveOsMemory => mi_option_reserve_os_memory,
         Self::ArenaReserve => mi_option_arena_reserve,
         Self::ArenaPurgeMinSize => mi_option_arena_purge_min_size,
         Self::ArenaMaxObjectSize => mi_option_arena_max_object_size,
         Self::ArenaPurgeImmediateSize => mi_option_arena_purge_immediate_size,
         Self::GuardedMin => mi_option_guarded_min,
         Self::GuardedMax => mi_option_guarded_max,
         Self::GuardedSampleRate => mi_option_guarded_sample_rate,
         Self::ProfileAllocInterval => mi_option_profile_alloc_interval,
         Self::ProfileInuseInterval => mi_option_profile_inuse_interval,
         Self::ProfileSampleRate => mi_option_profile_sample_rate,
         Self::StatsMergeThreshold => mi_option_stats_merge_threshold,
      }
   }

   /// Converts `bytes` to the stored value, which mimalloc keeps in KiB for
   /// the arena options so it fits a 32-bit `long`.
   fn encode(self, bytes: usize) -> Result<c_long, TryFromIntError> {
      let value = match self {
         Self::ReserveOsMemory
         | Self::ArenaReserve
         | Self::ArenaPurgeMinSize
         | Self::ArenaMaxObjectSize
         | Self::ArenaPurgeImmediateSize => bytes.div_ceil(1024),
         Self::GuardedMin
         | Self::GuardedMax
         | Self::GuardedSampleRate
         | Self::ProfileAllocInterval
         | Self::ProfileInuseInterval
         | Self::ProfileSampleRate
         | Self::StatsMergeThreshold => bytes,
      };

      c_long::try_from(value)
   }

   /// Returns the current size in bytes.
   #[inline]
   #[must_use = "this only reads the option"]
   pub fn get(self) -> usize {
      // SAFETY: `raw` is a valid option.
      unsafe { mi_option_get_size(self.raw()) }
   }

   /// Sets the size in bytes, rounded up to whole KiB for the arena options.
   ///
   /// # Errors
   ///
   /// Returns an error if the stored value does not fit a C `long`, which is
   /// 32 bits on Windows.
   ///
   /// # Safety
   ///
   /// No other thread may run mimalloc code during the call.
   #[inline]
   pub unsafe fn set(self, bytes: usize) -> Result<(), TryFromIntError> {
      let value = self.encode(bytes)?;

      // SAFETY: the caller guarantees no concurrent mimalloc calls.
      unsafe { mi_option_set(self.raw(), value) }
      Ok(())
   }

   /// Sets the size in bytes unless code or the environment already set it.
   ///
   /// # Errors
   ///
   /// Same as [`set`](Self::set).
   ///
   /// # Safety
   ///
   /// No other thread may run mimalloc code during the call.
   #[inline]
   pub unsafe fn set_default(self, bytes: usize) -> Result<(), TryFromIntError> {
      let value = self.encode(bytes)?;

      // SAFETY: the caller guarantees no concurrent mimalloc calls.
      unsafe { mi_option_set_default(self.raw(), value) }
      Ok(())
   }
}

impl Number {
   /// The `mi_option_t` value for this option.
   const fn raw(self) -> mi_option_t {
      match self {
         Self::Verbose => mi_option_verbose,
         Self::ArenaEagerCommit => mi_option_arena_eager_commit,
         Self::ReserveHugeOsPages => mi_option_reserve_huge_os_pages,
         Self::ReserveHugeOsPagesAt => mi_option_reserve_huge_os_pages_at,
         Self::PurgeDelay => mi_option_purge_delay,
         Self::UseNumaNodes => mi_option_use_numa_nodes,
         Self::OsTag => mi_option_os_tag,
         Self::MaxErrors => mi_option_max_errors,
         Self::MaxWarnings => mi_option_max_warnings,
         Self::ArenaPurgeMult => mi_option_arena_purge_mult,
         Self::RetryOnOom => mi_option_retry_on_oom,
         Self::GuardedSampleSeed => mi_option_guarded_sample_seed,
         Self::GenericCollect => mi_option_generic_collect,
         Self::PageReclaimOnFree => mi_option_page_reclaim_on_free,
         Self::PageFullRetain => mi_option_page_full_retain,
         Self::PageMaxCandidates => mi_option_page_max_candidates,
         Self::MaxVabits => mi_option_max_vabits,
         Self::PageCommitOnDemand => mi_option_page_commit_on_demand,
         Self::PageMaxReclaim => mi_option_page_max_reclaim,
         Self::PageCrossThreadMaxReclaim => mi_option_page_cross_thread_max_reclaim,
         Self::AllowThp => mi_option_allow_thp,
         Self::ProfileTimeInterval => mi_option_profile_time_interval,
      }
   }

   /// Returns the current value.
   #[inline]
   #[must_use = "this only reads the option"]
   pub fn get(self) -> c_long {
      // SAFETY: `raw` is a valid option.
      unsafe { mi_option_get(self.raw()) }
   }

   /// Sets the value.
   ///
   /// # Safety
   ///
   /// No other thread may run mimalloc code during the call.
   #[inline]
   pub unsafe fn set(self, value: c_long) {
      // SAFETY: the caller guarantees no concurrent mimalloc calls.
      unsafe { mi_option_set(self.raw(), value) }
   }

   /// Sets the value unless code or the environment already set it.
   ///
   /// # Safety
   ///
   /// No other thread may run mimalloc code during the call.
   #[inline]
   pub unsafe fn set_default(self, value: c_long) {
      // SAFETY: the caller guarantees no concurrent mimalloc calls.
      unsafe { mi_option_set_default(self.raw(), value) }
   }
}
