#![allow(clippy::disallowed_methods, reason = "build scripts are exempt")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use std::{env, io, path::PathBuf, process::Command};

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return Ok(());
    }

    println!("cargo:rerun-if-env-changed=SDKROOT");
    let sdk_path = match env::var("SDKROOT") {
        Ok(sdk_path) => sdk_path,
        Err(_) => {
            let output = Command::new("xcrun")
                .args(["--sdk", "macosx", "--show-sdk-path"])
                .output()?;
            if !output.status.success() {
                return Err(io::Error::other(format!(
                    "xcrun could not locate the macOS SDK: {}",
                    String::from_utf8_lossy(&output.stderr).trim_end()
                ))
                .into());
            }
            String::from_utf8(output.stdout)?.trim_end().to_owned()
        }
    };

    println!("cargo:rerun-if-changed=src/bindings.h");
    let mut bindings = bindgen::Builder::default()
        .header("src/bindings.h")
        .clang_arg(format!("-isysroot{sdk_path}"))
        .clang_arg("-xobjective-c")
        .allowlist_type("CMItemIndex")
        .allowlist_type("CMSampleTimingInfo")
        .allowlist_type("CMVideoCodecType")
        .allowlist_type("VTEncodeInfoFlags")
        .allowlist_function("CMTimeMake")
        .allowlist_var("kCVPixelFormatType_.*")
        .allowlist_var("kCVReturn.*")
        .allowlist_var("VTEncodeInfoFlags_.*")
        .allowlist_var("kCMVideoCodecType_.*")
        .allowlist_var("kCMTime.*")
        .allowlist_var("kCMSampleAttachmentKey_.*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .layout_tests(false);
    if let Ok(target) = env::var("TARGET") {
        bindings = bindings.clang_arg(format!("--target={target}"));
    }
    let bindings = bindings
        .generate()
        .map_err(|error| io::Error::other(format!("unable to generate bindings: {error}")))?;

    let out_path = PathBuf::from(env::var("OUT_DIR")?);
    bindings.write_to_file(out_path.join("bindings.rs"))?;

    Ok(())
}
