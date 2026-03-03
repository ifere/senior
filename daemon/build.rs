fn main() {
    let lib_dir = std::env::var("CACTUS_LIB_DIR").unwrap_or_else(|_| {
        println!(
            "cargo:warning=CACTUS_LIB_DIR is not set. \
             Set it to the directory containing libcactus.dylib (macOS) or libcactus.so (Linux). \
             Example: CACTUS_LIB_DIR=/path/to/cactus/build cargo build --release"
        );
        String::new()
    });
    if !lib_dir.is_empty() {
        println!("cargo:rustc-link-search={}", lib_dir);
        println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
    }
    println!("cargo:rustc-link-lib=dylib=cactus");
    println!("cargo:rerun-if-env-changed=CACTUS_LIB_DIR");
}
