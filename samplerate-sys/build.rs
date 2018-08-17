extern crate cc;

fn main() {
    if cfg!(feature = "system") {
        println!("cargo:rustc-link-lib=samplerate");
    } else {
        let version = env!("CARGO_PKG_VERSION").split("+").next().unwrap();
        cc::Build::new()
            // First, do what autoconf would do, but only for feature flags that are
            // actually used somewhere.
            .include("src")
            // We can safely assume we have C99.
            .define("HAVE_STDINT_H", "1")
            .define("HAVE_LRINT", "1")
            .define("HAVE_LRINTF", "1")
            // These are safe defaults.
            .define("CPU_CLIPS_NEGATIVE", "0")
            .define("CPU_CLIPS_POSITIVE", "0")
            // Package name and version.
            .define("PACKAGE", "\"libsamplerate\"")
            .define("VERSION", &format!("\"{}\"", version)[..])

            // Second, actually build the library.
            .flag_if_supported("-Wno-implicit-fallthrough")
            .include("vendor")
            .file("vendor/src_linear.c")
            .file("vendor/src_sinc.c")
            .file("vendor/src_zoh.c")
            .file("vendor/samplerate.c")
            .compile("samplerate");
    }
}
