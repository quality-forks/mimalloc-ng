#![allow(bad_style, clippy::all, reason = "ctest generates this code")]

use mimalloc_ng_sys::*;

include!(concat!(env!("OUT_DIR"), "/all.rs"));
