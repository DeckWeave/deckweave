//! Bounded raster/SVG rendering shared by strip composition and Encoder feedback.
use crate::Renderer;
use base64::{Engine, engine::general_purpose::STANDARD};
use image::{ImageReader, RgbaImage};
use serde_json::Value;
use std::io::Cursor;

impl Renderer {
    pub fn data_image(&self, uri: &str, width: u32, height: u32) -> Result<RgbaImage, String> {
        if width == 0 || height == 0 || width > 2048 || height > 2048 || uri.len() > 2 * 1024 * 1024
        {
            return Err("Artwork exceeds bounds".into());
        }
        let (header, data) = uri.split_once(',').ok_or("Expected image data URI")?;
        if !header.starts_with("data:image/") || !header.ends_with(";base64") {
            return Err("Expected base64 image".into());
        }
        let bytes = STANDARD.decode(data).map_err(|e| e.to_string())?;
        if header.contains("svg+xml") {
            if bytes.len() > 256 * 1024 || bytes.iter().filter(|b| **b == b'<').count() > 8192 {
                return Err("SVG exceeds bounds".into());
            }
            let tree = resvg::usvg::Tree::from_data(&bytes, &self.svg_options)
                .map_err(|e| e.to_string())?;
            let size = tree.size();
            if size.width() > 2048.0 || size.height() > 2048.0 {
                return Err("SVG viewport exceeds bounds".into());
            }
            let mut pix =
                resvg::tiny_skia::Pixmap::new(width, height).ok_or("Invalid SVG canvas")?;
            let scale = (width as f32 / size.width()).min(height as f32 / size.height());
            let transform = resvg::tiny_skia::Transform::from_scale(scale, scale).post_translate(
                (width as f32 - size.width() * scale) / 2.0,
                (height as f32 - size.height() * scale) / 2.0,
            );
            resvg::render(&tree, transform, &mut pix.as_mut());
            let mut rgba = pix.data().to_vec();
            for p in rgba.as_chunks_mut::<4>().0 {
                let a = u32::from(p[3]);
                for v in &mut p[..3] {
                    *v = (u32::from(*v) * 255).checked_div(a).unwrap_or(0).min(255) as u8;
                }
            }
            return RgbaImage::from_raw(width, height, rgba).ok_or("Invalid SVG raster".into());
        }
        let mut reader = ImageReader::new(Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(2048);
        limits.max_image_height = Some(2048);
        limits.max_alloc = Some(32 * 1024 * 1024);
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|e| e.to_string())?
            .resize(width, height, image::imageops::FilterType::Lanczos3)
            .to_rgba8();
        let mut canvas = RgbaImage::new(width, height);
        image::imageops::overlay(
            &mut canvas,
            &image,
            i64::from((width - image.width()) / 2),
            i64::from((height - image.height()) / 2),
        );
        Ok(canvas)
    }
    pub(crate) fn composite(
        &self,
        rgb: &mut [u8],
        stride: usize,
        rect: [u16; 4],
        image: &RgbaImage,
        opacity: f64,
    ) {
        let [x, y, w, h] = rect;
        for py in 0..h {
            for px in 0..w {
                let p = image.get_pixel(u32::from(px), u32::from(py));
                let a = (f64::from(p[3]) * opacity.clamp(0.0, 1.0)) as u32;
                let offset = ((usize::from(y + py) * stride) + usize::from(x + px)) * 3;
                for c in 0..3 {
                    rgb[offset + c] = ((u32::from(p[c]) * a
                        + u32::from(rgb[offset + c]) * (255 - a))
                        / 255) as u8;
                }
            }
        }
    }
    pub fn image_data_jpeg(&self, uri: &str) -> Result<Vec<u8>, String> {
        let image = self.data_image(uri, 112, 112)?;
        let mut rgb = vec![18; 112 * 112 * 3];
        self.composite(&mut rgb, 112, [0, 0, 112, 112], &image, 1.0);
        super::render::encode(&rgb, 112, 112, 10240)
    }
    pub fn image_data_png(&self, uri: &str, size: [u32; 2]) -> Result<Vec<u8>, String> {
        let image = self.data_image(uri, size[0], size[1])?;
        let mut png = Cursor::new(Vec::new());
        image
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(png.into_inner())
    }
    pub fn feedback_png(&self, layout: &Value, values: &Value) -> Result<Vec<u8>, String> {
        let items = layout["items"].as_array().ok_or("Layout has no items")?;
        if items.len() > 128 {
            return Err("Too many feedback items".into());
        }
        let mut rgb = vec![18; 200 * 100 * 3];
        let mut items: Vec<_> = items.iter().collect();
        items.sort_by_key(|i| i["zOrder"].as_i64().unwrap_or(0));
        for item in items {
            let value = &values[item["key"].as_str().unwrap_or("")];
            let mut item = item.clone();
            if let Some(object) = value.as_object() {
                for (key, value) in object {
                    item[key] = value.clone();
                }
            } else if !value.is_null() {
                item["value"] = value.clone();
            }
            if item["enabled"].as_bool() == Some(false) {
                continue;
            }
            let r = item["rect"]
                .as_array()
                .filter(|r| r.len() == 4)
                .ok_or("Invalid feedback rect")?;
            let r: Vec<u16> = r
                .iter()
                .map(|v| {
                    v.as_u64()
                        .filter(|n| *n <= 200)
                        .map(|n| n as u16)
                        .ok_or("Invalid feedback bounds")
                })
                .collect::<Result<_, _>>()?;
            let [x, y, w, h] = [r[0], r[1], r[2], r[3]];
            if w == 0 || h == 0 || x + w > 200 || y + h > 100 {
                return Err("Feedback bounds exceed 200x100".into());
            }
            match item["type"].as_str().unwrap_or("") {
                "pixmap" => {
                    if let Some(uri) = item["value"].as_str().filter(|s| !s.is_empty()) {
                        let image = self.data_image(uri, u32::from(w), u32::from(h))?;
                        self.composite(
                            &mut rgb,
                            200,
                            [x, y, w, h],
                            &image,
                            item["opacity"].as_f64().unwrap_or(1.0),
                        );
                    }
                }
                "text" => {
                    let s = item["value"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| {
                            if item["value"].is_null() {
                                String::new()
                            } else {
                                item["value"].to_string()
                            }
                        });
                    let s: String = s.chars().take(160).collect();
                    let s = s
                        .replace('&', "&amp;")
                        .replace('<', "&lt;")
                        .replace('>', "&gt;")
                        .replace('"', "&quot;");
                    let size = item["font"]["size"]
                        .as_f64()
                        .unwrap_or(18.0)
                        .clamp(4.0, 100.0);
                    let weight = item["font"]["weight"]
                        .as_u64()
                        .unwrap_or(400)
                        .clamp(100, 900);
                    let (anchor, tx) = match item["alignment"].as_str() {
                        Some("left") => ("start", 0.0),
                        Some("right") => ("end", f64::from(w)),
                        _ => ("middle", f64::from(w) / 2.0),
                    };
                    let [r, g, b] = color(&item["color"], [235, 235, 235]);
                    let svg = format!(
                        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\"><text x=\"{tx}\" y=\"{}\" font-family=\"IBM Plex Sans\" font-size=\"{size}\" font-weight=\"{weight}\" text-anchor=\"{anchor}\" fill=\"#{r:02x}{g:02x}{b:02x}\">{s}</text></svg>",
                        f64::from(h) / 2.0 + size * 0.35
                    );
                    let image = self.data_image(
                        &format!("data:image/svg+xml;base64,{}", STANDARD.encode(svg)),
                        u32::from(w),
                        u32::from(h),
                    )?;
                    self.composite(
                        &mut rgb,
                        200,
                        [x, y, w, h],
                        &image,
                        item["opacity"].as_f64().unwrap_or(1.0),
                    );
                }
                "bar" | "gbar" => {
                    let min = item["range"]["min"].as_f64().unwrap_or(0.0);
                    let max = item["range"]["max"].as_f64().unwrap_or(100.0);
                    let fill = if max > min {
                        ((item["value"].as_f64().unwrap_or(min) - min) / (max - min))
                            .clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let gbar = item["type"] == "gbar";
                    let bh = if gbar {
                        item["bar_h"].as_u64().unwrap_or(8).clamp(1, u64::from(h)) as u16
                    } else {
                        h
                    };
                    let border = item["border_w"]
                        .as_u64()
                        .unwrap_or(2)
                        .min(u64::from(bh / 2))
                        .min(u64::from(w / 2)) as u16;
                    let subtype = item["subtype"].as_u64().unwrap_or(0);
                    let opacity = item["opacity"].as_f64().unwrap_or(1.0).clamp(0.0, 1.0);
                    for py in 0..h {
                        for px in 0..w {
                            let t = f64::from(px) / f64::from(w.max(1));
                            let c = if py < bh {
                                if subtype == 2 || subtype == 3 {
                                    let trim = ((1.0 - t) * f64::from(bh) / 2.0) as u16;
                                    if py < trim || py >= bh - trim {
                                        continue;
                                    }
                                }
                                if px < border
                                    || px >= w - border
                                    || py < border
                                    || py >= bh - border
                                {
                                    color(&item["border_c"], [150, 150, 150])
                                } else {
                                    let filled = if subtype == 1 || subtype == 3 {
                                        (t - 0.5).abs() <= (fill - 0.5).abs()
                                            && ((t >= 0.5) == (fill >= 0.5))
                                    } else {
                                        t < fill
                                    };
                                    color_at(
                                        if filled {
                                            &item["bar_fill_c"]
                                        } else {
                                            &item["bar_bg_c"]
                                        },
                                        t,
                                        if filled { [80, 160, 230] } else { [45, 48, 55] },
                                    )
                                }
                            } else if gbar {
                                let center = (fill * f64::from(w - 1)) as i32;
                                let dy = py - bh;
                                let triangle = (h - bh).saturating_sub(dy) as i32;
                                if (i32::from(px) - center).abs() > triangle {
                                    continue;
                                }
                                color(&item["indicator_c"], [235, 235, 235])
                            } else {
                                continue;
                            };
                            let offset = ((usize::from(y + py) * 200) + usize::from(x + px)) * 3;
                            for i in 0..3 {
                                rgb[offset + i] = (f64::from(c[i]) * opacity
                                    + f64::from(rgb[offset + i]) * (1.0 - opacity))
                                    as u8;
                            }
                        }
                    }
                }
                _ => return Err("Unsupported feedback item type".into()),
            }
        }
        let image = image::RgbImage::from_raw(200, 100, rgb).ok_or("Invalid feedback image")?;
        let mut png = Cursor::new(Vec::new());
        image
            .write_to(&mut png, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(png.into_inner())
    }
}
fn color(value: &Value, fallback: [u8; 3]) -> [u8; 3] {
    let Some(s) = value
        .as_str()
        .and_then(|s| s.strip_prefix('#'))
        .filter(|s| s.len() == 6 || s.len() == 8)
    else {
        return fallback;
    };
    let Ok(n) = u32::from_str_radix(&s[..6], 16) else {
        return fallback;
    };
    [(n >> 16) as u8, (n >> 8) as u8, n as u8]
}
fn color_at(value: &Value, t: f64, fallback: [u8; 3]) -> [u8; 3] {
    let Some(s) = value.as_str().filter(|s| s.contains(':')) else {
        return color(value, fallback);
    };
    let mut stops: Vec<_> = s
        .split(',')
        .take(16)
        .filter_map(|part| {
            let (at, c) = part.split_once(':')?;
            let at = at.parse::<f64>().ok().filter(|v| v.is_finite())?;
            Some((at, color(&Value::String(c.into()), fallback)))
        })
        .collect();
    stops.sort_by(|a, b| a.0.total_cmp(&b.0));
    let Some(first) = stops.first() else {
        return fallback;
    };
    if t <= first.0 {
        return first.1;
    }
    for pair in stops.windows(2) {
        let [(a, ca), (b, cb)] = pair else {
            continue;
        };
        if t <= *b {
            let f = if b > a { (t - a) / (b - a) } else { 1.0 };
            return [0, 1, 2].map(|i| (f64::from(ca[i]) * (1.0 - f) + f64::from(cb[i]) * f) as u8);
        }
    }
    stops.last().map_or(fallback, |s| s.1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn svg_retains_aspect_and_png_alpha_without_external_file_loading() {
        let r = Renderer::new().unwrap();
        let uri=format!("data:image/svg+xml;base64,{}",STANDARD.encode(r##"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="50"><rect width="50" height="50" fill="#f00"/><image href="file:///C:/Windows/Web/Wallpaper/Windows/img0.jpg" width="100" height="50"/></svg>"##));
        let image = r.data_image(&uri, 200, 100).unwrap();
        assert_eq!(image.get_pixel(10, 50).0, [255, 0, 0, 255]);
        assert_eq!(image.get_pixel(180, 50).0, [0, 0, 0, 0]);
        assert!(r.image_data_jpeg(&uri).unwrap().len() <= 10240);
    }
    #[test]
    fn feedback_updates_render_pixels_and_clip_invalid_bounds() {
        let r = Renderer::new().unwrap();
        let layout = json!({"items":[{"key":"level","type":"bar","rect":[0,0,200,100],"border_w":0,"bar_bg_c":"#000000","bar_fill_c":"#ff0000"}]});
        let png = r.feedback_png(&layout, &json!({"level":25})).unwrap();
        let image = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(image.get_pixel(20, 50).0, [255, 0, 0]);
        assert_eq!(image.get_pixel(100, 50).0, [0, 0, 0]);
        assert!(
            r.feedback_png(
                &json!({"items":[{"type":"text","rect":[190,0,20,100]}]}),
                &json!({})
            )
            .is_err()
        );
    }
}
