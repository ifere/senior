fn main() {
    let lib_dir = std::env::var("CACTUS_LIB_DIR")
        .unwrap_or_else(|_| "/Users/chilly/dev/cactus/cactus/build".to_string());
    println!("cargo:rustc-link-search={}", lib_dir);
    println!("cargo:rustc-link-lib=dylib=cactus");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
    println!("cargo:rerun-if-env-changed=CACTUS_LIB_DIR");
}
