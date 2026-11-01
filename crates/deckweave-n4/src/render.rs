use fontdue::{
    Font, FontSettings,
    layout::{CoordinateSystem, HorizontalAlign, Layout, LayoutSettings, TextStyle, VerticalAlign},
};
use jpeg_encoder::{ColorType, Encoder};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyVisual {
    pub title: String,
    pub value: String,
    pub active: bool,
    pub pressed: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegionVisual {
    pub rect: [u16; 4],
    pub visual: KeyVisual,
    pub detail: String,
    pub image: Option<String>,
}

pub struct Renderer {
    font: Font,
    pub(crate) svg_options: resvg::usvg::Options<'static>,
}
impl Renderer {
    pub fn image_jpeg(&self, bytes: &[u8]) -> Result<Vec<u8>, String> {
        let reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .map_err(|e| e.to_string())?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(2048);
        limits.max_image_height = Some(2048);
        limits.max_alloc = Some(32 * 1024 * 1024);
        let mut reader = reader;
        reader.limits(limits);
        let image = reader
            .decode()
            .map_err(|e| e.to_string())?
            .resize_exact(112, 112, image::imageops::FilterType::Triangle)
            .to_rgba8();
        let rgb: Vec<u8> = image
            .pixels()
            .flat_map(|p| {
                let a = u32::from(p[3]);
                [0, 1, 2].map(|c| ((u32::from(p[c]) * a + 18 * (255 - a)) / 255) as u8)
            })
            .collect();
        encode(&rgb, 112, 112, 10240)
    }
    pub fn new() -> Result<Self, String> {
        let mut svg_options = resvg::usvg::Options::default();
        svg_options
            .fontdb_mut()
            .load_font_data(include_bytes!("../assets/IBMPlexSans.ttf").to_vec());
        svg_options.font_family = "IBM Plex Sans".into();
        svg_options.image_href_resolver = resvg::usvg::ImageHrefResolver {
            resolve_string: Box::new(|_, _| None),
            resolve_data: Box::new(|_, _, _| None),
        };
        Ok(Self {
            svg_options,
            font: Font::from_bytes(
                include_bytes!("../assets/IBMPlexSans.ttf") as &[u8],
                FontSettings::default(),
            )
            .map_err(str::to_owned)?,
        })
    }
    /// The exact rotated JPEG is used by BAT and the preview (which rotates it back in CSS).
    pub fn jpeg(&self, visual: &KeyVisual) -> Result<Vec<u8>, String> {
        let bg = if visual.pressed {
            [69, 101, 151]
        } else if visual.active {
            [39, 65, 96]
        } else {
            [18, 22, 30]
        };
        let mut rgb = vec![0; 112 * 112 * 3];
        for pixel in rgb.as_chunks_mut::<3>().0 {
            pixel.copy_from_slice(&bg)
        }
        self.text(&mut rgb, 112, [5, 12, 102, 58], &visual.value, 36.0);
        self.text(&mut rgb, 112, [5, 76, 102, 28], &visual.title, 13.0);
        encode(&rgb, 112, 112, 10240)
    }
    pub fn strip(&self, regions: &[RegionVisual]) -> Result<Vec<u8>, String> {
        if regions.len() > 64 {
            return Err("Too many strip regions".into());
        }
        let mut rgb = vec![12; 800 * 128 * 3];
        for region in regions {
            let [x, y, width, height] = region.rect;
            if width < 12
                || height < 12
                || u32::from(x) + u32::from(width) > 800
                || u32::from(y) + u32::from(height) > 128
            {
                return Err("Invalid strip region".into());
            }
            let bg = if region.visual.pressed {
                [69, 101, 151]
            } else if region.visual.active {
                [39, 65, 96]
            } else {
                [18, 22, 30]
            };
            for py in y..y + height {
                for px in x..x + width {
                    let offset = (usize::from(py) * 800 + usize::from(px)) * 3;
                    rgb[offset..offset + 3].copy_from_slice(&bg);
                }
            }
            if let Some(image) = &region.image
                && let Ok(image) = self.data_image(image, u32::from(width), u32::from(height))
            {
                self.composite(&mut rgb, 800, [x, y, width, height], &image, 1.0);
                continue;
            }
            self.text(
                &mut rgb,
                800,
                [x + 4, y + 3, width - 8, height / 3],
                &region.visual.value,
                28.0,
            );
            self.text(
                &mut rgb,
                800,
                [x + 4, y + height / 3 + 3, width - 8, height / 4],
                &region.visual.title,
                18.0,
            );
            self.text(
                &mut rgb,
                800,
                [x + 4, y + height * 2 / 3, width - 8, height / 4],
                &region.detail,
                16.0,
            );
        }
        encode(&rgb, 800, 128, 128 * 1024)
    }
    pub(crate) fn text(
        &self,
        rgb: &mut [u8],
        canvas_width: usize,
        bounds: [u16; 4],
        text: &str,
        size: f32,
    ) {
        let [left, top, width, height] = bounds;
        let canvas_height = rgb.len() / (canvas_width * 3);
        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: f32::from(left),
            y: f32::from(top),
            max_width: Some(f32::from(width)),
            max_height: Some(f32::from(height)),
            horizontal_align: HorizontalAlign::Center,
            vertical_align: VerticalAlign::Middle,
            ..Default::default()
        });
        let bounded: String = text.chars().take(40).collect();
        layout.append(&[&self.font], &TextStyle::new(&bounded, size, 0));
        for glyph in layout.glyphs() {
            let (_, bitmap) = self.font.rasterize_config(glyph.key);
            for gy in 0..glyph.height {
                for gx in 0..glyph.width {
                    let x = glyph.x.floor() as i32 + gx as i32;
                    let py = glyph.y.floor() as i32 + gy as i32;
                    if x < i32::from(left)
                        || py < i32::from(top)
                        || x >= i32::from(left + width)
                        || py >= i32::from(top + height)
                        || x < 0
                        || py < 0
                        || x as usize >= canvas_width
                        || py as usize >= canvas_height
                    {
                        continue;
                    }
                    let alpha = u32::from(bitmap[gy * glyph.width + gx]);
                    let offset = (py as usize * canvas_width + x as usize) * 3;
                    for c in 0..3 {
                        rgb[offset + c] = ((u32::from(rgb[offset + c]) * (255 - alpha)
                            + 235 * alpha)
                            / 255) as u8;
                    }
                }
            }
        }
    }
}

pub(crate) fn encode(rgb: &[u8], width: u16, height: u16, limit: usize) -> Result<Vec<u8>, String> {
    let rotated: Vec<u8> = rgb
        .as_chunks::<3>()
        .0
        .iter()
        .rev()
        .flatten()
        .copied()
        .collect();
    for quality in [85, 65, 45] {
        let mut bytes = vec![];
        Encoder::new(&mut bytes, quality)
            .encode(&rotated, width, height, ColorType::Rgb)
            .map_err(|e| e.to_string())?;
        if bytes.len() <= limit {
            return Ok(bytes);
        }
    }
    Err("N4 JPEG exceeds the transfer limit".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strip_is_bounded_and_invalid_regions_are_rejected() {
        let renderer = Renderer::new().unwrap();
        let region = RegionVisual {
            image: None,
            rect: [0, 0, 160, 128],
            visual: KeyVisual {
                title: "Область".into(),
                value: "ON".into(),
                active: true,
                pressed: false,
            },
            detail: "D1: -50".into(),
        };
        let jpeg = renderer.strip(std::slice::from_ref(&region)).unwrap();
        assert!(jpeg.len() <= 128 * 1024);
        let mut bad = region;
        bad.rect = [799, 0, 160, 128];
        assert!(renderer.strip(&[bad]).is_err());
    }
    #[test]
    fn cyrillic_and_long_titles_produce_bounded_jpeg_for_transport() {
        let renderer = Renderer::new().unwrap();
        for title in ["Проверка", &"Я".repeat(80), ""] {
            let image = renderer
                .jpeg(&KeyVisual {
                    title: title.into(),
                    value: i64::MIN.to_string(),
                    active: true,
                    pressed: false,
                })
                .unwrap();
            assert!(image.len() <= 10240);
            assert_eq!(&image[..2], &[0xff, 0xd8]);
            assert_eq!(&image[image.len() - 2..], &[0xff, 0xd9]);
            let mut reports = vec![];
            crate::packets::key_image(1, &image, |r| {
                reports.push(r.to_vec());
                Ok(())
            })
            .unwrap();
            assert!(reports.len() >= 3);
        }
    }
}
