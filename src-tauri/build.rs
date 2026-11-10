fn main() {
    println!("cargo:rerun-if-env-changed=YANMEW_BUILD_ID");
    if let Ok(build)=std::env::var("YANMEW_BUILD_ID") { println!("cargo:rustc-env=YANMEW_BUILD_ID={build}"); }
    tauri_build::build()
}
