//! Compiles the C shim against a local LibRaw build (snapshot 202611 or later, the first with the Nikon HE decoder).
//! `LIBRAW_DIR` points at a LibRaw checkout built with `make -f Makefile.dist lib/libraw.a`; default
//! `~/Documents/_vendor/LibRaw`. LibRaw is LGPL-2.1 / CDDL-1.0 and never enters this repository.

use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-env-changed=LIBRAW_DIR");
    println!("cargo:rerun-if-changed=shim.c");
    let dir = std::env::var_os("LIBRAW_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(std::env::var_os("HOME").expect("HOME")).join("Documents/_vendor/LibRaw"));
    let lib = dir.join("lib/libraw.a");
    assert!(
        lib.is_file(),
        "nikon-he: {} not found; build LibRaw (snapshot 202611+) with `make -f Makefile.dist lib/libraw.a` or set LIBRAW_DIR",
        lib.display()
    );
    println!("cargo:rerun-if-changed={}", lib.display());
    cc::Build::new().file("shim.c").include(&dir).define("LIBRAW_NOTHREADS", None).warnings(false).compile("lc_nikon_he_shim");
    println!("cargo:rustc-link-search=native={}", dir.join("lib").display());
    println!("cargo:rustc-link-lib=static=raw");
    println!("cargo:rustc-link-lib=z");
    let target = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    println!("cargo:rustc-link-lib={}", if target == "macos" { "c++" } else { "stdc++" });
}
