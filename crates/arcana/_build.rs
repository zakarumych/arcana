fn main() {
    if cfg!(feature = "ed") {
        println!("cargo:rustc-cfg=arcana_ed");
    }
    if cfg!(feature = "client") {
        println!("cargo:rustc-cfg=arcana_client");
    }
    if cfg!(feature = "server") {
        println!("cargo:rustc-cfg=arcana_server");
    }
}
