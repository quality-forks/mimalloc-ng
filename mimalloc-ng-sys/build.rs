use std::{
   env,
   error::Error,
   fs,
   path::{
      Path,
      PathBuf,
   },
};

/// Whether the cargo feature `name` is enabled.
fn feature(name: &str) -> bool {
   env::var_os(format!("CARGO_FEATURE_{}", name.to_uppercase())).is_some()
}

/// The value of `cfg(target_<name>)`, empty when unset.
fn cfg(name: &str) -> String {
   env::var(format!("CARGO_CFG_TARGET_{}", name.to_uppercase())).unwrap_or_default()
}

/// Cargo-provided directory variable `name`.
fn dir(name: &str) -> Result<PathBuf, Box<dyn Error>> {
   env::var_os(name)
      .map(PathBuf::from)
      .ok_or_else(|| format!("{name} is not set").into())
}

fn main() -> Result<(), Box<dyn Error>> {
   let manifest_dir = dir("CARGO_MANIFEST_DIR")?;
   let out_dir = dir("OUT_DIR")?;
   let root = manifest_dir.join("mimalloc");
   let include = root.join("include");
   let static_c = root.join("src").join("static.c");

   println!("cargo::rerun-if-changed=mimalloc");
   println!("cargo::metadata=include_dir={}", include.display());

   let os = cfg("os");
   let family = cfg("family");
   let target_env = cfg("env");
   let vendor = cfg("vendor");
   let arch = cfg("arch");
   let target_features = cfg("feature");
   let has_feature = |name: &str| target_features.split(',').any(|feature| feature == name);
   let profile_debug = env::var("DEBUG").is_ok_and(|value| value == "true");

   let mut build = cc::Build::new();
   build.include(&include).include(root.join("src"));

   if target_env == "msvc" {
      // Upstream CMake forces C++ on MSVC and clang-cl to get std::atomic
      // (MI_USE_CXX).
      let wrapper = out_dir.join("static.cc");
      fs::write(
         &wrapper,
         format!("#include \"{}\"\n", forward_slashes(&static_c)),
      )?;
      build
         .cpp(true)
         .std("c++17")
         .flag_if_supported("/Zc:__cplusplus")
         .file(wrapper);
   } else {
      build
         .file(&static_c)
         .flag_if_supported("-Wno-error=date-time");
   }

   if feature("override") {
      if family != "windows" {
         build.define("MI_MALLOC_OVERRIDE", None);
      }

      if vendor == "apple" {
         build
            .define("MI_OSX_ZONE", "1")
            .define("MI_OSX_INTERPOSE", "1");
      }

      if !build.get_compiler().is_like_msvc() {
         build.flag_if_supported("-fno-builtin-malloc");
      }
   }

   if feature("secure") {
      build.define("MI_SECURE", "4");
   }

   if os == "windows" && feature("win_direct_tls") {
      build.define("MI_WIN_DIRECT_TLS", "1");
   }

   if target_env == "musl" {
      build.define("MI_LIBC_MUSL", "1");
   }

   if family == "unix" && os != "haiku" {
      // musl needs local-dynamic for static builds, see microsoft/mimalloc#644.
      let model = if feature("local_dynamic_tls") || target_env == "musl" {
         "local-dynamic"
      } else {
         "initial-exec"
      };
      build.flag_if_supported(format!("-ftls-model={model}"));
   }

   if feature("free_use_pagemap") {
      build.define("MI_FREE_USE_PAGEMAP", "1");
   }

   if feature("skip_collect_on_exit") {
      build.define("MI_SKIP_COLLECT_ON_EXIT", None);
   }

   let execinfo = vendor == "apple"
      || (os == "linux" && target_env == "gnu")
      || matches!(os.as_str(), "freebsd" | "dragonfly");

   configure_profiler(&mut build, feature("profiler"), family == "unix", execinfo);

   // Rust only enables LSE atomics when the whole target may assume ARMv8.1.
   // Apple's default CPU already implies it.
   if arch == "aarch64" && vendor != "apple" && has_feature("lse") {
      if build.get_compiler().is_like_msvc() {
         build.flag("/arch:armv8.1");
      } else {
         build.flag("-march=armv8.1-a");
      }
   }

   if feature("debug") {
      build.define("MI_DEBUG", "3").define("MI_SHOW_ERRORS", "1");
   } else {
      build.define("MI_DEBUG", "0");

      if !profile_debug {
         build
            .define("MI_BUILD_RELEASE", None)
            .define("NDEBUG", None);
      }
   }

   build.compile("mimalloc");

   link_system_libraries(
      &os,
      &arch,
      has_feature("v7"),
      feature("profiler") && execinfo,
   );
   Ok(())
}

/// Builds the heap profiler with the platform support upstream `CMake` would
/// detect, or compiles it out.
fn configure_profiler(build: &mut cc::Build, enabled: bool, unix: bool, execinfo: bool) {
   if !enabled {
      build.define("MI_PROFILE", "0");
      return;
   }

   // Without these headers the profiler records no stacks and cannot write
   // profile files.
   if unix {
      build.define("MI_HAS_UNISTDH", "1");
   }

   if execinfo {
      build.define("MI_HAS_EXECINFOH", "1");
   }
}

/// Links the system libraries that upstream `CMake` adds for `os`.
fn link_system_libraries(os: &str, arch: &str, armv7: bool, execinfo: bool) {
   if execinfo && matches!(os, "freebsd" | "dragonfly") {
      println!("cargo::rustc-link-lib=execinfo");
   }

   // ARMv6 has no 64-bit atomic instructions, ARMv7 does.
   if os == "linux" && arch == "arm" && !armv7 {
      let atomic = env::var("DEP_ATOMIC").unwrap_or_else(|_| "atomic".to_owned());
      println!("cargo::rustc-link-lib={atomic}");
   }

   if os == "windows" {
      for lib in ["psapi", "shell32", "user32", "advapi32", "bcrypt"] {
         println!("cargo::rustc-link-lib={lib}");
      }
   }
}

/// `path` with `/` separators, for a C `#include` that MSVC and clang-cl both
/// accept.
fn forward_slashes(path: &Path) -> String {
   path.to_string_lossy().replace('\\', "/")
}
