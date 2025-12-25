use std::{env, path::PathBuf};

fn main() {
    // Rebuild if icon or rc changes
    println!("cargo:rerun-if-changed=../logo.ico");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let rc_file = out_dir.join("launcher.rc");
    std::fs::write(&rc_file, r#"IDI_ICON1 ICON "../logo.ico"\n"#)
        .expect("Failed to write launcher.rc");
    let res_file = out_dir.join("launcher.res");

    println!("cargo:rustc-link-arg={}", res_file.display());

    let tool =
        cc::windows_registry::find_tool(&env::var("CARGO_CFG_TARGET_ARCH").unwrap(), "rc.exe")
            .expect("rc.exe not found in PATH");

    // Call rc.exe directly
    let status = tool
        .to_command()
        .args(["/fo".as_ref(), res_file.as_os_str(), rc_file.as_os_str()])
        .status()
        .expect("failed to run rc.exe");

    if !status.success() {
        panic!("Resource compilation failed");
    }

    let logo = image::load(
        std::io::BufReader::new(
            std::fs::File::open("../logo256.png").expect("Failed to open logo256.png"),
        ),
        image::ImageFormat::Png,
    )
    .expect("Failed to load logo");

    let logo = logo.into_rgba8();

    let raw_logo_file = out_dir.join("logo256.raw");

    std::fs::write(&raw_logo_file, logo.into_raw()).expect("Failed to write raw logo");
}
