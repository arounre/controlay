use std::env;
use std::fs::File;
use std::io::Write;
use std::path::Path;

fn main() {
    println!("cargo:rerun-if-changed=assets/logos/app_icon_circle.png");

    // Icon setup for EGUI
    let icon_bytes = include_bytes!("./assets/logos/app_icon_circle.png");
    let image = image::load_from_memory_with_format(icon_bytes, image::ImageFormat::Png)
        .expect("Failed to load icon image in build script");

    let image = image.into_rgba8();
    let (width, height) = image.dimensions();
    let rgba_data = image.into_raw();

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir);

    let mut rgba_file = File::create(dest_path.join("icon.rgba")).unwrap();
    rgba_file.write_all(&rgba_data).unwrap();

    let mut meta_file = File::create(dest_path.join("icon_meta.rs")).unwrap();
    writeln!(meta_file, "pub const ICON_WIDTH: u32 = {};", width).unwrap();
    writeln!(meta_file, "pub const ICON_HEIGHT: u32 = {};", height).unwrap();

    // Icon setup for windows
    if cfg!(target_os = "windows") {
        let mut res = winres::WindowsResource::new();
        res.set_icon("assets/logos/app_icon_circle.ico");
        res.compile().unwrap();
    }

    println!("Processed icon: {}x{}", width, height);
}
