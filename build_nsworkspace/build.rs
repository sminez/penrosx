use std::{env, io::Error, path::Path, process::Command};

fn get_sdk_path() -> Result<String, Error> {
    let output = Command::new("xcrun")
        .args(["--sdk", "macosx", "--show-sdk-path"])
        .output()?
        .stdout;

    let output_str = String::from_utf8(output).expect("Failed to convert xcrun output to string");

    Ok(output_str.trim().to_string())
}

pub fn main() {
    let target = env::var("TARGET").unwrap();
    let default_sdk_path = "/Applications/Xcode.app/Contents/Developer/Platforms/MacOSX.platform/Developer/SDKs/MacOSX12.1.sdk";

    let sdk_path: String = match get_sdk_path() {
        Ok(path) => path,
        Err(e) => {
            println!(
                "cargo:warning=Failed to get MacOSX SDK Path. Trying to use default one. {e:?}",
            );
            String::from(default_sdk_path)
        }
    };

    println!("cargo:rustc-link-lib=framework=AppKit");

    let builder = bindgen::Builder::default()
        .header_contents(
            "NSWorkspace.h",
            "
            #include<AppKit/NSWorkspace.h>
            #include<AppKit/NSRunningApplication.h>
        ",
        )
        .clang_arg(format!("--target={target}"))
        .clang_args(&["-isysroot", sdk_path.as_ref()])
        .wrap_unsafe_ops(true)
        // .block_extern_crate(true)
        // .objc_extern_crate(true)
        .clang_arg("-ObjC")
        .blocklist_item("objc_object");

    let bindings = builder.generate().expect("Failed to generate bindings");

    bindings
        .write_to_file(Path::new("../src").join("nsworkspace.rs"))
        .expect("Failed to write bindings to file");
}
