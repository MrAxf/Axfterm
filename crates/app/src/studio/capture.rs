//! Captura de la ventana GPUI a PNG.

use std::{
    io::BufWriter,
    path::{Path, PathBuf},
};

use axf_ui::gpui::Window;

use super::protocol::{Clip, ShotInfo};

/// Captura el frame actual y escribe un PNG.
pub(super) fn capture(
    window: &mut Window,
    shots_dir: &Path,
    name: &str,
    clip: Option<Clip>,
    ui_scale: f32,
) -> anyhow::Result<ShotInfo> {
    let image = window.render_to_image()?;
    let scale = window.scale_factor();
    let (width, height) = image.dimensions();
    let raw = image.into_raw();
    let mut shot = write_png(shots_dir, name, width, height, scale, raw, clip)?;
    shot.ui_scale = ui_scale;
    Ok(shot)
}

fn write_png(
    shots_dir: &Path,
    name: &str,
    width: u32,
    height: u32,
    scale: f32,
    raw: Vec<u8>,
    clip: Option<Clip>,
) -> anyhow::Result<ShotInfo> {
    let (image_width, image_height, pixels, logical_width, logical_height) = match clip {
        Some(clip) => {
            let (x, y, w, h) = physical_clip(clip, width, height, scale)?;
            let row = (width as usize) * 4;
            let mut cropped = Vec::with_capacity((w as usize) * (h as usize) * 4);
            for row_index in 0..h as usize {
                let source = (y as usize + row_index) * row + (x as usize) * 4;
                let end = source + (w as usize) * 4;
                cropped.extend_from_slice(&raw[source..end]);
            }
            (w, h, cropped, w as f32 / scale, h as f32 / scale)
        }
        None => (
            width,
            height,
            raw,
            width as f32 / scale,
            height as f32 / scale,
        ),
    };
    anyhow::ensure!(image_width > 0 && image_height > 0, "captura vacía");

    let file_name = sanitize_name(name)?;
    let path = shots_dir.join(format!("{file_name}.png"));
    let temporary = shots_dir.join(format!(".{file_name}.png.tmp.{}", std::process::id()));
    write_atomic(&temporary, &path, image_width, image_height, &pixels)?;
    Ok(ShotInfo {
        path: path.display().to_string(),
        width: image_width,
        height: image_height,
        logical_width,
        logical_height,
        scale,
        ui_scale: 1.0,
    })
}

fn physical_clip(
    clip: Clip,
    width: u32,
    height: u32,
    scale: f32,
) -> anyhow::Result<(u32, u32, u32, u32)> {
    anyhow::ensure!(
        clip.width > 0.0 && clip.height > 0.0,
        "el recorte debe tener ancho y alto positivos"
    );
    let x = (clip.x * scale).round().max(0.0) as u32;
    let y = (clip.y * scale).round().max(0.0) as u32;
    let w = (clip.width * scale).round().max(1.0) as u32;
    let h = (clip.height * scale).round().max(1.0) as u32;
    let x = x.min(width.saturating_sub(1));
    let y = y.min(height.saturating_sub(1));
    let w = w.min(width - x);
    let h = h.min(height - y);
    Ok((x, y, w, h))
}

fn sanitize_name(name: &str) -> anyhow::Result<String> {
    let mut result = String::with_capacity(name.len());
    for character in name.chars() {
        if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
            result.push(character.to_ascii_lowercase());
        } else if !result.ends_with('-') {
            result.push('-');
        }
    }
    let result = result.trim_matches(['-', '.']).to_owned();
    anyhow::ensure!(!result.is_empty(), "nombre de captura vacío");
    anyhow::ensure!(
        !result.contains(".."),
        "el nombre de captura no puede contener `..`"
    );
    Ok(result)
}

fn write_atomic(
    temporary: &PathBuf,
    destination: &Path,
    width: u32,
    height: u32,
    pixels: &[u8],
) -> anyhow::Result<()> {
    anyhow::ensure!(
        pixels.len() == (width as usize) * (height as usize) * 4,
        "el buffer RGBA no coincide con las dimensiones"
    );
    let file = std::fs::File::create(temporary)?;
    {
        let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header()?;
        writer.write_image_data(pixels)?;
        writer.finish()?;
    }
    let _ = std::fs::remove_file(destination);
    std::fs::rename(temporary, destination)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_become_safe_file_stems() {
        assert_eq!(
            sanitize_name("Gallery Actions").expect("nombre válido"),
            "gallery-actions"
        );
        assert_eq!(sanitize_name("../x").expect("nombre válido"), "x");
        assert!(sanitize_name("a..b").is_err());
        assert!(sanitize_name("///").is_err());
    }

    #[test]
    fn clip_is_scaled_and_clamped() {
        assert_eq!(
            physical_clip(
                Clip {
                    x: 0.0,
                    y: 0.0,
                    width: 10.0,
                    height: 10.0
                },
                20,
                20,
                2.0
            )
            .expect("recorte válido"),
            (0, 0, 20, 20)
        );
        assert_eq!(
            physical_clip(
                Clip {
                    x: 5.0,
                    y: 5.0,
                    width: 100.0,
                    height: 100.0
                },
                20,
                20,
                1.0
            )
            .expect("recorte válido"),
            (5, 5, 15, 15)
        );
        assert!(
            physical_clip(
                Clip {
                    x: 0.0,
                    y: 0.0,
                    width: 0.0,
                    height: 1.0
                },
                20,
                20,
                1.0
            )
            .is_err()
        );
    }
}
