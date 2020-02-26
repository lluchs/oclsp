use regex::Regex;
use std::io;
use std::io::prelude::*;
use std::fs::File;
use std::env;

fn main() {
    let cmakelists = read_file("openclonk/CMakeLists.txt").unwrap();
    // We need to patch the openclonk CMakeLists.txt slightly to make the build work.
    let cmakelists_patched = {
        // Don't search for audio libraries to avoid having to specify the audio include path for
        // the glue code.
        let c = Regex::new(r#"(?m)^find_package\("Audio"\)$"#).unwrap()
            .replace(&cmakelists, "#$0");
        // Image libraries aren't required, so remove that dependency.
        let c = Regex::new(r#"(?m)^find_package\((JPEG|PNG) REQUIRED\)$"#).unwrap()
            .replace_all(&c, "#$0");
        // Don't require native c4group when cross-compiling.
        let c = Regex::new(r#"(?m)^[^#\n]*IMPORT_NATIVE_TOOLS.*$"#).unwrap()
            .replace_all(&c, "#$0");
        c.into_owned()
    };
    if cmakelists_patched != cmakelists {
        write_file("openclonk/CMakeLists.txt", &cmakelists_patched).unwrap();
    }

    // Build libmisc and libc4script via cmake.
    let mut cmake_cfg = cmake::Config::new("openclonk");
    cmake_cfg.define("HEADLESS_ONLY", "ON");
    let cmake_dst = cmake_cfg.build_target("libmisc").build();
    cmake_cfg.build_target("libc4script").build();
    cmake_cfg.build_target("blake2").build();
    println!("cargo:rerun-if-changed=openclonk"); // note: will not apply to code changes inside

    // Manually build stub files
    let mut cfg = cc::Build::new();
    cfg.cpp(true);
    cfg.include("openclonk/src")
        .include("openclonk/include")
        .include("openclonk/thirdparty")
        .include(format!("{}/build", cmake_dst.display()))
        .define("HAVE_CONFIG_H", Some("1"))
        .file("openclonk/src/script/C4ScriptStandalone.cpp")
        .file("openclonk/src/script/C4ScriptStandaloneStubs.cpp");

    if env::var("PROFILE").unwrap() == "debug" {
        cfg.define("_DEBUG", Some("1"));
    }
    cfg.compile("c4scriptstubs");

    println!("cargo:rustc-link-search=native={}/build", cmake_dst.display());
    println!("cargo:rustc-link-search=native={}/build/thirdparty/blake2", cmake_dst.display());
    println!("cargo:rustc-link-lib=static=libc4script");
    println!("cargo:rustc-link-lib=static=libmisc");
    println!("cargo:rustc-link-lib=static=blake2");
    println!("cargo:rustc-link-lib=z");

    if env::var("TARGET").unwrap().contains("windows") {
        println!("cargo:rustc-link-lib=winmm");
    }

}

fn read_file(path: &str) -> io::Result<String> {
    let mut contents = String::new();
    File::open(path)?.read_to_string(&mut contents)?;
    Ok(contents)
}

fn write_file(path: &str, contents: &str) -> io::Result<()> {
    File::create(path)?
        .write_all(contents.as_bytes())
}
