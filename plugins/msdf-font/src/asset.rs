use arcana::assets::{Asset, AssetBuilder, AssetError, AssetVersion, Assets};
use std::sync::Arc;

pub(crate) const MAX_DIM: u32 = 4096;
pub(crate) const MAX_GLYPHS: usize = 4096;
const MAGIC: &[u8; 8] = b"ARCFONT\0";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FieldKind {
    Sdf,
    #[default]
    Msdf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FontMetrics {
    pub ascender: f32,
    pub descender: f32,
    pub line_height: f32,
    pub underline_y: f32,
    pub underline_thickness: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Glyph {
    pub character: char,
    pub glyph_id: u16,
    /// Horizontal advance in em units.
    pub advance: f32,
    /// Left, bottom, right, top in em units, including field padding.
    pub plane_bounds: [f32; 4],
    /// X, Y, width, height in the top-down atlas; zero for an empty outline.
    pub atlas_bounds: [u32; 4],
}

#[derive(Clone, Debug)]
pub struct GlyphBitmap {
    pub width: u32,
    pub height: u32,
    /// Grayscale coverage, one byte per pixel, top-down.
    pub pixels: Vec<u8>,
    /// Top-left pixel position relative to the baseline; Y points down.
    pub offset: [i32; 2],
    pub advance: f32,
}

#[derive(Debug, thiserror::Error)]
pub enum FontError {
    #[error("Invalid font asset: {0}")]
    Invalid(&'static str),
    #[error("Invalid font source: {0}")]
    Source(String),
    #[error("Font has no glyph for {0:?}")]
    MissingGlyph(char),
    #[error("Font atlas or raster exceeds configured limits")]
    Limit,
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

#[derive(Debug)]
pub(crate) struct FontData {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub kind: FieldKind,
    pub distance_range: f32,
    pub pixels_per_em: f32,
    pub metrics: FontMetrics,
    pub glyphs: Vec<Glyph>,
}

#[derive(Clone, Debug, arcana::HasStid)]
#[stid = "arcana-msdf-font::Font"]
pub struct Font(Arc<FontData>);

impl AssetVersion for Font {
    fn version() -> u64 {
        1
    }
}
impl Asset for Font {
    type Loaded = Self;
    async fn load(data: Box<[u8]>, _: &Assets) -> Result<Self, AssetError> {
        Self::from_bytes(&data).map_err(AssetError::new)
    }
    fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
        Ok(loaded)
    }
}

impl Font {
    pub(crate) fn new(data: FontData) -> Result<Self, FontError> {
        Self::validate(&data, data.pixels.len())?;
        Ok(Self(Arc::new(data)))
    }

    fn validate(data: &FontData, pixel_len: usize) -> Result<(), FontError> {
        if data.width == 0
            || data.height == 0
            || data.width > MAX_DIM
            || data.height > MAX_DIM
            || data.glyphs.len() > MAX_GLYPHS
        {
            return Err(FontError::Limit);
        }
        if pixel_len != data.width as usize * data.height as usize * 4 {
            return Err(FontError::Invalid("pixel length"));
        }
        if !data.distance_range.is_finite()
            || !(1.0..=64.0).contains(&data.distance_range)
            || !data.pixels_per_em.is_finite()
            || !(1.0..=512.0).contains(&data.pixels_per_em)
        {
            return Err(FontError::Invalid("field scale"));
        }
        let m = &data.metrics;
        if [
            m.ascender,
            m.descender,
            m.line_height,
            m.underline_y,
            m.underline_thickness,
        ]
        .iter()
        .any(|x| !x.is_finite() || x.abs() > 1024.0)
        {
            return Err(FontError::Invalid("metrics"));
        }
        let mut prev = None;
        for g in &data.glyphs {
            if prev.is_some_and(|c| c >= g.character) {
                return Err(FontError::Invalid("glyph order"));
            }
            prev = Some(g.character);
            if !g.advance.is_finite()
                || g.advance.abs() > 1024.0
                || g.plane_bounds
                    .iter()
                    .any(|x| !x.is_finite() || x.abs() > 1024.0)
            {
                return Err(FontError::Invalid("glyph metrics"));
            }
            let [x, y, w, h] = g.atlas_bounds;
            if w == 0 || h == 0 {
                if g.atlas_bounds != [0; 4] || g.plane_bounds != [0.0; 4] {
                    return Err(FontError::Invalid("empty glyph"));
                }
            } else if x.checked_add(w).is_none_or(|v| v > data.width)
                || y.checked_add(h).is_none_or(|v| v > data.height)
                || g.plane_bounds[0] >= g.plane_bounds[2]
                || g.plane_bounds[1] >= g.plane_bounds[3]
            {
                return Err(FontError::Invalid("glyph bounds"));
            }
            if ((g.plane_bounds[2] - g.plane_bounds[0]) * data.pixels_per_em - w as f32).abs()
                > 0.01
                || ((g.plane_bounds[3] - g.plane_bounds[1]) * data.pixels_per_em - h as f32).abs()
                    > 0.01
            {
                return Err(FontError::Invalid("glyph scale"));
            }
        }
        Ok(())
    }
    pub fn width(&self) -> u32 {
        self.0.width
    }
    pub fn height(&self) -> u32 {
        self.0.height
    }
    pub fn pixels(&self) -> &[u8] {
        &self.0.pixels
    }
    pub fn kind(&self) -> FieldKind {
        self.0.kind
    }
    pub fn distance_range(&self) -> f32 {
        self.0.distance_range
    }
    pub fn pixels_per_em(&self) -> f32 {
        self.0.pixels_per_em
    }
    pub fn metrics(&self) -> &FontMetrics {
        &self.0.metrics
    }
    pub fn glyphs(&self) -> &[Glyph] {
        &self.0.glyphs
    }
    pub fn glyph(&self, c: char) -> Option<&Glyph> {
        self.0
            .glyphs
            .binary_search_by_key(&c, |g| g.character)
            .ok()
            .map(|i| &self.0.glyphs[i])
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, FontError> {
        let mut out = MAGIC.to_vec();
        let mut word = |v: u32| out.extend_from_slice(&v.to_le_bytes());
        word(1);
        word(match self.kind() {
            FieldKind::Sdf => 0,
            FieldKind::Msdf => 1,
        });
        word(self.width());
        word(self.height());
        word(self.distance_range().to_bits());
        word(self.pixels_per_em().to_bits());
        let m = self.metrics();
        for v in [
            m.ascender,
            m.descender,
            m.line_height,
            m.underline_y,
            m.underline_thickness,
        ] {
            word(v.to_bits());
        }
        word(self.glyphs().len() as u32);
        for g in self.glyphs() {
            word(g.character as u32);
            word(g.glyph_id as u32);
            word(g.advance.to_bits());
            for v in g.plane_bounds {
                word(v.to_bits());
            }
            for v in g.atlas_bounds {
                word(v);
            }
        }
        out.extend_from_slice(self.pixels());
        Ok(out)
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, FontError> {
        if bytes.get(..8) != Some(MAGIC.as_slice()) {
            return Err(FontError::Invalid("magic"));
        }
        let mut r = Reader { bytes: &bytes[8..] };
        if r.u32()? != 1 {
            return Err(FontError::Invalid("version"));
        }
        let kind = match r.u32()? {
            0 => FieldKind::Sdf,
            1 => FieldKind::Msdf,
            _ => return Err(FontError::Invalid("field kind")),
        };
        let width = r.u32()?;
        let height = r.u32()?;
        if width == 0 || height == 0 || width > MAX_DIM || height > MAX_DIM {
            return Err(FontError::Limit);
        }
        let distance_range = r.f32()?;
        let pixels_per_em = r.f32()?;
        let metrics = FontMetrics {
            ascender: r.f32()?,
            descender: r.f32()?,
            line_height: r.f32()?,
            underline_y: r.f32()?,
            underline_thickness: r.f32()?,
        };
        let count = r.u32()? as usize;
        if count > MAX_GLYPHS {
            return Err(FontError::Limit);
        }
        let pixel_len = width as usize * height as usize * 4;
        if r.bytes.len() != count * 44 + pixel_len {
            return Err(FontError::Invalid("file length"));
        }
        let mut glyphs = Vec::with_capacity(count);
        for _ in 0..count {
            let character = char::from_u32(r.u32()?).ok_or(FontError::Invalid("character"))?;
            let glyph_id = u16::try_from(r.u32()?).map_err(|_| FontError::Invalid("glyph id"))?;
            let advance = r.f32()?;
            let plane_bounds = [r.f32()?, r.f32()?, r.f32()?, r.f32()?];
            let atlas_bounds = [r.u32()?, r.u32()?, r.u32()?, r.u32()?];
            glyphs.push(Glyph {
                character,
                glyph_id,
                advance,
                plane_bounds,
                atlas_bounds,
            });
        }
        let mut data = FontData {
            width,
            height,
            pixels: Vec::new(),
            kind,
            distance_range,
            pixels_per_em,
            metrics,
            glyphs,
        };
        Self::validate(&data, r.bytes.len())?;
        data.pixels = r.bytes.to_vec();
        Ok(Self(Arc::new(data)))
    }

    pub fn rasterize_glyph(
        &self,
        character: char,
        pixels_per_em: f32,
    ) -> Result<GlyphBitmap, FontError> {
        if !pixels_per_em.is_finite() || !(1.0..=4096.0).contains(&pixels_per_em) {
            return Err(FontError::Invalid("raster scale"));
        }
        let g = self
            .glyph(character)
            .ok_or(FontError::MissingGlyph(character))?;
        let [left, bottom, right, top] = g.plane_bounds;
        let x0 = (left * pixels_per_em).floor();
        let y0 = (-top * pixels_per_em).floor();
        let width = ((right * pixels_per_em).ceil() - x0) as u32;
        let height = ((-bottom * pixels_per_em).ceil() - y0) as u32;
        if width > MAX_DIM || height > MAX_DIM {
            return Err(FontError::Limit);
        }
        let mut out = GlyphBitmap {
            width,
            height,
            pixels: vec![0; width as usize * height as usize],
            offset: [x0 as i32, y0 as i32],
            advance: g.advance * pixels_per_em,
        };
        let [ax, ay, aw, ah] = g.atlas_bounds;
        if aw == 0 {
            return Ok(out);
        }
        let range = (self.distance_range() * pixels_per_em / self.pixels_per_em()).max(1.0);
        for y in 0..height {
            for x in 0..width {
                let sx = (((x0 + x as f32 + 0.5) / pixels_per_em - left) / (right - left)
                    * aw as f32
                    - 0.5)
                    .clamp(0.0, (aw - 1) as f32);
                let sy = ((top + (y0 + y as f32 + 0.5) / pixels_per_em) / (top - bottom)
                    * ah as f32
                    - 0.5)
                    .clamp(0.0, (ah - 1) as f32);
                let ix = sx.floor() as u32;
                let iy = sy.floor() as u32;
                let tx = sx - ix as f32;
                let ty = sy - iy as f32;
                let mut rgb = [0.0; 3];
                for (c, value) in rgb.iter_mut().enumerate() {
                    let sample = |x: u32, y: u32| {
                        self.pixels()[((ay + y) as usize * self.width() as usize
                            + (ax + x) as usize)
                            * 4
                            + c] as f32
                            / 255.0
                    };
                    let a = sample(ix, iy) * (1.0 - tx) + sample((ix + 1).min(aw - 1), iy) * tx;
                    let b = sample(ix, (iy + 1).min(ah - 1)) * (1.0 - tx)
                        + sample((ix + 1).min(aw - 1), (iy + 1).min(ah - 1)) * tx;
                    *value = a * (1.0 - ty) + b * ty;
                }
                let d = match self.kind() {
                    FieldKind::Sdf => rgb[0],
                    FieldKind::Msdf => rgb[0].min(rgb[1]).max(rgb[0].max(rgb[1]).min(rgb[2])),
                };
                out.pixels[(y * width + x) as usize] =
                    (((d - 0.5) * range + 0.5).clamp(0.0, 1.0) * 255.0).round() as u8;
            }
        }
        Ok(out)
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
}
impl Reader<'_> {
    fn u32(&mut self) -> Result<u32, FontError> {
        let a = self
            .bytes
            .get(..4)
            .ok_or(FontError::Invalid("truncated file"))?;
        let v = u32::from_le_bytes([a[0], a[1], a[2], a[3]]);
        self.bytes = &self.bytes[4..];
        Ok(v)
    }
    fn f32(&mut self) -> Result<f32, FontError> {
        Ok(f32::from_bits(self.u32()?))
    }
}
