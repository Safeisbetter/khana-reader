// Builds the small native layers (CoreText, Apple Vision, CTranslate2, LAME) and links the libraries
// copied from the Python app (vendor/lib: the same Vosk and CTranslate2 builds it uses).
// For Android (android/build.sh) only CTranslate2's layer and LAME are built here, with the NDK;
// CTranslate2 itself is android/build/ct2 (android/build_ct2.sh), Vosk and ONNX Runtime the
// libraries in android/build/jniLibs.
fn main() {
    let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let android = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android");
    let ios = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("ios");
    // Windows: no native layer (no translator, no MP3 maker yet): ONNX Runtime's onnxruntime.dll is loaded at run time
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        return;
    }
    // `shell`: a phone, whose Java or Swift shell gives the folders, the models and the ONNX Runtime library at start
    println!("cargo:rustc-check-cfg=cfg(shell)");
    if android || ios {
        println!("cargo:rustc-cfg=shell");
    }
    if ios {
        // The iPhone: the small native layers are built here; CTranslate2 and ONNX Runtime are linked by the shell's build
        // (ios/build.sh), so nothing is asked for here that does not exist yet.
        cc::Build::new().file("native/typeset.c").opt_level(2).compile("typeset");
        cc::Build::new().file("native/vision.m").flag("-fobjc-arc").warnings(false).opt_level(2).compile("vision_shim");
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .file("native/ct2_shim.cpp")
            .include(root.join("vendor/ct2include"))
            .warnings(false)
            .opt_level(2)
            .compile("ct2_shim");
        lame(&root, false);
        for f in ["CoreText", "Vision", "Foundation", "CoreGraphics"] {
            println!("cargo:rustc-link-lib=framework={f}");
        }
        println!("cargo:rerun-if-changed=native");
        return;
    }
    if android {
        let ct2 = root.join("android/build/ct2");
        cc::Build::new()
            .cpp(true)
            .std("c++17")
            .file("native/ct2_shim.cpp")
            .include(root.join("android/toolchain/ct2-src/include"))
            .cpp_link_stdlib(None)
            .flag(prefix_map(&root))
            .warnings(false)
            .opt_level(2)
            .compile("ct2_shim");
        println!("cargo:rustc-link-search=native={}", ct2.display());
        println!("cargo:rustc-link-lib=static=ctranslate2");
        println!("cargo:rustc-link-lib=static=omp");
        // (the NDK's C++ library inside libkhana.so: Vosk and ONNX Runtime carry their own)
        println!("cargo:rustc-link-lib=static=c++_static");
        println!("cargo:rustc-link-lib=static=c++abi");
        println!("cargo:rustc-link-lib=dylib=log");
        println!("cargo:rustc-link-search=native={}", root.join("android/build/jniLibs/arm64-v8a").display());
        // (16 KB pages: phones with Android 15 and newer may use them, and then load only libraries
        // aligned for them, as Vosk's and ONNX Runtime's are)
        println!("cargo:rustc-link-arg=-Wl,-z,max-page-size=16384");
        println!("cargo:rerun-if-changed=native/ct2_shim.cpp");
        println!("cargo:rerun-if-changed=android/build/ct2/libctranslate2.a");
        lame(&root, true);
        return;
    }
    let lib = root.join("vendor/lib");
    // the translated PDF, laid out by CoreText
    cc::Build::new().file("native/typeset.c").opt_level(2).compile("typeset");
    println!("cargo:rustc-link-lib=framework=CoreText");
    println!("cargo:rerun-if-changed=native/typeset.c");
    // Apple Vision text recognition
    cc::Build::new().file("native/vision.m").flag("-fobjc-arc").opt_level(2).compile("vision_shim");
    for f in ["Vision", "Foundation", "CoreGraphics"] {
        println!("cargo:rustc-link-lib=framework={f}");
    }
    println!("cargo:rerun-if-changed=native/vision.m");
    // CTranslate2 4.5.0 (translation), through a small C++ layer
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .file("native/ct2_shim.cpp")
        .include(root.join("vendor/ct2include"))
        .warnings(false)
        .opt_level(2)
        .compile("ct2_shim");
    println!("cargo:rustc-link-lib=dylib=ctranslate2");
    println!("cargo:rustc-link-lib=dylib=c++");
    println!("cargo:rerun-if-changed=native/ct2_shim.cpp");
    lame(&root, false);
    println!("cargo:rustc-link-search=native={}", lib.display());
    // during development the libraries are found in vendor/lib; in the app bundle in Frameworks
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", lib.display());
    println!("cargo:rustc-link-arg=-Wl,-rpath,@executable_path/../Frameworks");
}

/// LAME 3.100, the MP3 encoder inside the Python app's lameenc (library part only)
fn lame(root: &std::path::Path, android: bool) {
    let mut lame = cc::Build::new();
    if android {
        lame.flag(prefix_map(root));
    }
    for e in std::fs::read_dir(root.join("vendor/lame/libmp3lame")).unwrap() {
        let p = e.unwrap().path();
        if p.extension().map_or(false, |x| x == "c") {
            lame.file(p);
        }
    }
    lame.include(root.join("vendor/lame"))
        .include(root.join("vendor/lame/include"))
        .include(root.join("vendor/lame/libmp3lame"))
        .define("HAVE_CONFIG_H", None)
        .warnings(false)
        .opt_level(3)
        .compile("mp3lame");
    println!("cargo:rerun-if-changed=vendor/lame");
}

/// (Android: this Mac's folder names left out of the library's file names, e.g. in assert messages,
/// so the build is the same wherever it is made)
fn prefix_map(root: &std::path::Path) -> String {
    format!("-ffile-prefix-map={}=/khana", root.display())
}
