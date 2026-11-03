//! Inspect actual hardware rasterization without opening a HID device.
use deckweave_n4::{KeyVisual, RegionVisual, Renderer};
use image::{ImageReader, RgbaImage};
use serde::Deserialize;
use std::{io::Cursor, path::PathBuf};

#[derive(Deserialize)]
struct Artwork {
    strip: Vec<String>,
    keys: Vec<String>,
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("Usage: render-widgets INPUT.json OUTPUT.png".into());
    }
    let data: Artwork = serde_json::from_slice(&std::fs::read(&args[0])?)?;
    if data.strip.len() != 3 || data.keys.len() > 12 {
        return Err("Expected three strip widgets and at most twelve keys".into());
    }
    let renderer = Renderer::new()?;
    let mut sheet = RgbaImage::from_pixel(800, 400, image::Rgba([65, 64, 61, 255]));
    let bounds = [[0, 0, 300, 128], [300, 0, 220, 128], [520, 0, 280, 128]];
    let regions: Vec<_> = data
        .strip
        .into_iter()
        .zip(bounds)
        .map(|(uri, rect)| RegionVisual {
            alert: false,
            rect,
            visual: KeyVisual {
                title: String::new(),
                value: String::new(),
                active: false,
                pressed: false,
            },
            detail: String::new(),
            image: Some(uri),
        })
        .collect();
    let strip = renderer.strip(&regions)?;
    let output = PathBuf::from(&args[1]);
    std::fs::write(output.with_extension("strip.jpg"), &strip)?;
    let image = image::imageops::rotate180(
        &ImageReader::new(Cursor::new(strip))
            .with_guessed_format()?
            .decode()?
            .to_rgba8(),
    );
    image::imageops::overlay(&mut sheet, &image, 0, 0);
    for (index, uri) in data.keys.iter().enumerate() {
        let bytes = renderer.image_data_jpeg(uri)?;
        let image = image::imageops::rotate180(
            &ImageReader::new(Cursor::new(bytes))
                .with_guessed_format()?
                .decode()?
                .to_rgba8(),
        );
        image::imageops::overlay(
            &mut sheet,
            &image,
            (index % 6) as i64 * 132,
            152 + (index / 6) as i64 * 124,
        );
    }
    sheet.save(output)?;
    Ok(())
}
