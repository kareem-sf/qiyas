//! Sets `cfg(pdfium)` when the pdfium library is there for the tests: `QIYAS_PDFIUM` is set, or
//! `scripts/pdfium.sh` has put it in `crates/qiyas-desktop/resources/pdfium/`. Tests that need
//! pdfium are ignored without it.

use std::path::Path;

fn main() {
    println!("cargo::rustc-check-cfg=cfg(pdfium)");
    println!("cargo::rerun-if-env-changed=QIYAS_PDFIUM");
    // At run time, not `env!`: a build script compiled for a copy of the repo sharing this
    // target folder would otherwise look in that copy's folder.
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo");
    let dir = Path::new(&manifest).join("../qiyas-desktop/resources/pdfium");
    println!("cargo::rerun-if-changed={}", dir.display());
    let name = format!(
        "{}pdfium{}",
        std::env::var("CARGO_CFG_TARGET_OS").map_or("lib", |os| if os == "windows" {
            ""
        } else {
            "lib"
        }),
        match std::env::var("CARGO_CFG_TARGET_OS").as_deref() {
            Ok("windows") => ".dll",
            Ok("macos") => ".dylib",
            _ => ".so",
        }
    );
    if std::env::var_os("QIYAS_PDFIUM").is_some() || dir.join(name).is_file() {
        println!("cargo::rustc-cfg=pdfium");
    }
}
