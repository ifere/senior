fn main() {
    let lib_dir = match std::env::var("CACTUS_LIB_DIR") {
        Ok(dir) => dir,
        Err(_) => {
            println!("cargo:warning=CACTUS_LIB_DIR not set — falling back to dev path. Set it before release builds.");
            "/Users/chilly/dev/cactus/cactus/build".to_string()
        }
    };
    println!("cargo:rustc-link-search={}", lib_dir);
    println!("cargo:rustc-link-lib=dylib=cactus");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib_dir);
    println!("cargo:rerun-if-env-changed=CACTUS_LIB_DIR");
}
