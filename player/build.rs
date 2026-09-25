fn main() {
    println!("cargo:rerun-if-changed=assets/icon/recurate.rc");
    println!("cargo:rerun-if-changed=assets/icon/recurate.ico");
    #[cfg(windows)]
    embed_resource::compile("assets/icon/recurate.rc", embed_resource::NONE)
        .manifest_optional()
        .unwrap();
}
