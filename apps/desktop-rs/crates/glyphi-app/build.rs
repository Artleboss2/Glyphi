//! Windows metadata: the icon Explorer, the taskbar and the installer read.
//!
//! Without this the executable carries the blank default mark, which is exactly what
//! a decorated window cannot supply on its own. On other platforms the icon comes
//! from the bundle, so there is nothing to do.

fn main() {
    println!("cargo:rerun-if-changed=assets/glyphi.ico");
    #[cfg(windows)]
    {
        let mut resource = winres::WindowsResource::new();
        resource.set_icon("assets/glyphi.ico");
        resource.set("FileDescription", "Glyphi");
        resource.set("ProductName", "Glyphi");
        resource.set("OriginalFilename", "glyphi.exe");
        resource
            .compile()
            .expect("the Windows icon could not be embedded");
    }
}
