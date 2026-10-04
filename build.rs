fn main() {
    println!("cargo:rerun-if-changed=assets/AppIcon.ico");
    #[cfg(target_os = "windows")]
    if std::env::var_os("CARGO_FEATURE_DESKTOP").is_some() {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/AppIcon.ico");
        resource.set("ProductName", "Open Usage");
        resource.set("FileDescription", "Open Usage - AI usage in view");
        resource
            .compile()
            .expect("Windows app resources could not be compiled");
    }
}
