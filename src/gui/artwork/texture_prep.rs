use std::path::Path;

use eframe::egui::{self, Context};

use super::{
    ArtworkTextures,
    MATTMC_BLUR_BACKGROUND_RGB,
    MATTMC_SVG_TARGET_MAX_DIMENSION,
    PREPARED_ARTWORK_MAX_HEIGHT,
    PREPARED_ARTWORK_MAX_WIDTH,
    EMULATOR_ARTWORK_MIN_HEIGHT,
    EMULATOR_ARTWORK_MIN_WIDTH,
    PreparedArtwork,
};

pub(super) fn prepare_artwork_payload_from_path(
    path: &Path,
    blur_background_rgb: Option<[u8; 3]>,
) -> Option<PreparedArtwork> {
    let image_bytes = std::fs::read(path).ok()?;
    let foreground_rgba = image::load_from_memory(&image_bytes).ok()?.to_rgba8();
    prepare_artwork_payload_from_rgba(foreground_rgba, blur_background_rgb)
}

pub(super) fn build_artwork_textures_from_payload(
    ctx: &Context,
    key: &str,
    payload: PreparedArtwork,
) -> Option<ArtworkTextures> {
    let foreground_color_image = egui::ColorImage::from_rgba_unmultiplied(
        [payload.width, payload.height],
        &payload.foreground_rgba,
    );
    let background_color_image = egui::ColorImage::from_rgba_unmultiplied(
        [payload.width, payload.height],
        &payload.background_rgba,
    );

    let foreground = ctx.load_texture(
        format!("game-artwork-fg-{}", key),
        foreground_color_image,
        egui::TextureOptions::LINEAR,
    );
    let background_blur = ctx.load_texture(
        format!("game-artwork-bg-{}", key),
        background_color_image,
        egui::TextureOptions::LINEAR,
    );

    Some(ArtworkTextures {
        foreground,
        _background_blur: background_blur,
    })
}

pub(super) fn load_artwork_textures_from_svg_bytes(
    ctx: &Context,
    key: &str,
    svg_bytes: &[u8],
) -> Option<ArtworkTextures> {
    let usvg_options = resvg::usvg::Options::default();
    let tree = resvg::usvg::Tree::from_data(svg_bytes, &usvg_options).ok()?;
    let base_size = tree.size().to_int_size();
    let largest_dim = base_size.width().max(base_size.height()) as f32;
    if largest_dim <= 0.0 {
        return None;
    }

    let scale = (MATTMC_SVG_TARGET_MAX_DIMENSION as f32 / largest_dim).max(1.0);
    let target_width = ((base_size.width() as f32) * scale).round().max(1.0) as u32;
    let target_height = ((base_size.height() as f32) * scale).round().max(1.0) as u32;

    let mut pixmap = resvg::tiny_skia::Pixmap::new(target_width, target_height)?;
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );

    let foreground_rgba = image::RgbaImage::from_raw(
        target_width,
        target_height,
        pixmap.data().to_vec(),
    )?;

    build_artwork_textures_from_rgba(
        ctx,
        key,
        foreground_rgba,
        Some(MATTMC_BLUR_BACKGROUND_RGB),
    )
}

pub(super) fn build_artwork_textures_from_rgba(
    ctx: &Context,
    key: &str,
    foreground_rgba: image::RgbaImage,
    blur_background_rgb: Option<[u8; 3]>,
) -> Option<ArtworkTextures> {
    let payload = prepare_artwork_payload_from_rgba(foreground_rgba, blur_background_rgb)?;
    build_artwork_textures_from_payload(ctx, key, payload)
}

pub(super) fn is_valid_portrait_artwork(path: &Path) -> bool {
    let Ok(image_reader) = image::ImageReader::open(path) else {
        return false;
    };

    let Ok((width, height)) = image_reader.into_dimensions() else {
        return false;
    };

    if width < 300 || height < 450 {
        return false;
    }

    let aspect_ratio = width as f32 / height as f32;
    (0.60..=0.74).contains(&aspect_ratio)
}

pub(super) fn is_valid_emulator_artwork(path: &Path) -> bool {
    let Ok(image_reader) = image::ImageReader::open(path) else {
        return false;
    };

    let Ok((width, height)) = image_reader.into_dimensions() else {
        return false;
    };

    width >= EMULATOR_ARTWORK_MIN_WIDTH && height >= EMULATOR_ARTWORK_MIN_HEIGHT
}

fn prepare_artwork_payload_from_rgba(
    foreground_rgba: image::RgbaImage,
    blur_background_rgb: Option<[u8; 3]>,
) -> Option<PreparedArtwork> {
    let foreground_rgba = resize_for_prepared_artwork(
        foreground_rgba,
        PREPARED_ARTWORK_MAX_WIDTH,
        PREPARED_ARTWORK_MAX_HEIGHT,
    );

    let background_rgba = if let Some(background_rgb) = blur_background_rgb {
        composite_on_solid_background(&foreground_rgba, background_rgb)
    } else {
        foreground_rgba.clone()
    };

    let width = usize::try_from(foreground_rgba.width()).ok()?;
    let height = usize::try_from(foreground_rgba.height()).ok()?;

    Some(PreparedArtwork {
        width,
        height,
        foreground_rgba: foreground_rgba.into_raw(),
        background_rgba: background_rgba.into_raw(),
    })
}

fn resize_for_prepared_artwork(
    rgba: image::RgbaImage,
    max_width: u32,
    max_height: u32,
) -> image::RgbaImage {
    let width = rgba.width();
    let height = rgba.height();

    if width == 0 || height == 0 || width <= max_width && height <= max_height {
        return rgba;
    }

    let width_scale = max_width as f32 / width as f32;
    let height_scale = max_height as f32 / height as f32;
    let scale = width_scale.min(height_scale).min(1.0);

    let resized_width = ((width as f32) * scale).round().max(1.0) as u32;
    let resized_height = ((height as f32) * scale).round().max(1.0) as u32;

    image::imageops::resize(
        &rgba,
        resized_width,
        resized_height,
        image::imageops::FilterType::Triangle,
    )
}

fn composite_on_solid_background(
    foreground: &image::RgbaImage,
    background_rgb: [u8; 3],
) -> image::RgbaImage {
    let mut output = image::RgbaImage::new(foreground.width(), foreground.height());

    for (x, y, pixel) in foreground.enumerate_pixels() {
        let alpha = pixel.0[3] as f32 / 255.0;
        let inv_alpha = 1.0 - alpha;

        let r = (pixel.0[0] as f32 * alpha + background_rgb[0] as f32 * inv_alpha).round() as u8;
        let g = (pixel.0[1] as f32 * alpha + background_rgb[1] as f32 * inv_alpha).round() as u8;
        let b = (pixel.0[2] as f32 * alpha + background_rgb[2] as f32 * inv_alpha).round() as u8;

        output.put_pixel(x, y, image::Rgba([r, g, b, 255]));
    }

    output
}
