pub fn main() {
    println!("cargo:rustc-link-search=framework=/System/Library/PrivateFrameworks");

    println!("cargo:rustc-link-lib=framework=AppKit");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
    println!("cargo:rustc-link-lib=framework=Foundation");
    println!("cargo:rustc-link-lib=framework=SkyLight");
}
