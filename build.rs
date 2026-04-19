use std::env;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;

use ico::{IconDir, IconDirEntry, IconImage, ResourceType};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};

fn main() {
    println!("cargo:rerun-if-changed=resources/assets/icons/basalt.svg");

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        return;
    }

    if let Err(error) = compile_windows_icon() {
        panic!("Failed to compile Windows icon resources: {error}");
    }
}

fn compile_windows_icon() -> Result<(), String> {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR")
        .map_err(|error| format!("CARGO_MANIFEST_DIR not set: {error}"))?;
    let out_dir = env::var("OUT_DIR").map_err(|error| format!("OUT_DIR not set: {error}"))?;

    let svg_path = Path::new(&manifest_dir).join("resources/assets/icons/basalt.svg");
    let ico_path = Path::new(&out_dir).join("basalt.ico");

    generate_ico_from_svg(&svg_path, &ico_path)?;

    #[cfg(windows)]
    {
        let icon_path = ico_path
            .to_str()
            .ok_or_else(|| "Generated icon path contains invalid UTF-8".to_owned())?;

        let mut windows_resource = winresource::WindowsResource::new();
        windows_resource.set_icon(icon_path);
        windows_resource
            .compile()
            .map_err(|error| format!("winresource compile failed: {error}"))?;
    }

    Ok(())
}

fn generate_ico_from_svg(svg_path: &Path, ico_path: &Path) -> Result<(), String> {
    let svg_bytes = std::fs::read(svg_path)
        .map_err(|error| format!("Failed to read {}: {error}", svg_path.display()))?;

    let usvg_options = Options::default();
    let tree = Tree::from_data(&svg_bytes, &usvg_options)
        .map_err(|error| format!("Failed to parse {}: {error}", svg_path.display()))?;

    let source_size = tree.size().to_int_size();
    let source_width = source_size.width() as f32;
    let source_height = source_size.height() as f32;

    let mut icon_dir = IconDir::new(ResourceType::Icon);
    for size in [16u32, 24, 32, 48, 64, 128, 256] {
        let mut pixmap = Pixmap::new(size, size)
            .ok_or_else(|| format!("Failed to allocate pixmap for {size}x{size}"))?;

        let scale = (size as f32 / source_width).min(size as f32 / source_height);
        let dx = ((size as f32) - (source_width * scale)) / 2.0;
        let dy = ((size as f32) - (source_height * scale)) / 2.0;

        let transform = Transform::from_scale(scale, scale).post_translate(dx, dy);
        resvg::render(&tree, transform, &mut pixmap.as_mut());

        let icon_image = IconImage::from_rgba_data(size, size, pixmap.data().to_vec());
        let icon_entry = IconDirEntry::encode(&icon_image)
            .map_err(|error| format!("Failed to encode icon entry {size}x{size}: {error}"))?;
        icon_dir.add_entry(icon_entry);
    }

    let writer = BufWriter::new(
        File::create(ico_path)
            .map_err(|error| format!("Failed to create {}: {error}", ico_path.display()))?,
    );
    icon_dir
        .write(writer)
        .map_err(|error| format!("Failed to write {}: {error}", ico_path.display()))?;

    Ok(())
}
