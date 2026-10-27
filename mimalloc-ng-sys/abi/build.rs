fn main() {
   let mut generator = ctest::TestGenerator::new();

   generator
      .edition(2024)
      .header("mimalloc.h")
      .header("mimalloc-stats.h")
      .include(concat!(env!("CARGO_MANIFEST_DIR"), "/../mimalloc/include"))
      .skip_signededness(|ty| ty == "mi_option_t")
      .rename_struct_ty(|ty| Some(ty.to_owned()))
      .rename_struct_field(|_, field| {
         matches!(
            field.ident(),
            "segments_reserved" | "stat_reserved" | "stat_counter_reserved"
         )
         .then(|| format!("_{}", field.ident()))
      })
      .rename_type(|ty| (ty.ends_with("_fun") || ty == "mi_commit_fun_t").then(|| format!("{ty}*")))
      .skip_struct(|item| matches!(item.ident(), "mi_heap_t" | "mi_theap_t"));

   ctest::generate_test(&mut generator, "../src/lib.rs", "all.rs").expect("generate ctest");
}
