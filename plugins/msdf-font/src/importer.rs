use crate::{
    FieldKind, Font, FontError, FontMetrics, Glyph,
    asset::{FontData, MAX_DIM, MAX_GLYPHS},
};
use arcana::{
    assets::import::{ImportContext, ImportError, Importer, ImporterDesc},
    model::{Model, Value},
};
use bymsdfgen_core::{
    Bitmap, Contour, DistanceMapping, EdgeSegment, Projection, Range, SdfTransformation, Shape,
    Vector2, generate_msdf, generate_sdf,
};
use serde::Deserialize;
use std::path::Path;
use ttf_parser::{Face, OutlineBuilder};

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImportConfig {
    pub characters: String,
    pub kind: FieldKind,
    pub pixels_per_em: f32,
    /// Full signed-distance span in atlas texels.
    pub distance_range: f32,
    pub atlas_width: u32,
    pub max_atlas_height: u32,
    pub face_index: u32,
}
impl Default for ImportConfig {
    fn default() -> Self {
        Self {
            characters: (' '..='~').collect(),
            kind: FieldKind::Msdf,
            pixels_per_em: 48.0,
            distance_range: 4.0,
            atlas_width: 512,
            max_atlas_height: 4096,
            face_index: 0,
        }
    }
}

#[derive(Default)]
pub struct FontImporter;

impl FontImporter {
    /// Imports requested Unicode characters; missing cmap entries are errors.
    pub fn import_bytes(&self, bytes: &[u8], config: &ImportConfig) -> Result<Font, FontError> {
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(FontError::Limit);
        }
        if !config.pixels_per_em.is_finite()
            || !(1.0..=512.0).contains(&config.pixels_per_em)
            || !config.distance_range.is_finite()
            || !(1.0..=64.0).contains(&config.distance_range)
            || config.atlas_width < 4
            || config.atlas_width > MAX_DIM
            || config.max_atlas_height < 4
            || config.max_atlas_height > MAX_DIM
            || config.characters.len() > MAX_GLYPHS * 4
        {
            return Err(FontError::Invalid("import configuration"));
        }
        let mut characters: Vec<_> = config.characters.chars().collect();
        characters.sort_unstable();
        characters.dedup();
        if characters.is_empty() || characters.len() > MAX_GLYPHS {
            return Err(FontError::Invalid("character count"));
        }
        let source =
            Face::parse(bytes, config.face_index).map_err(|e| FontError::Source(e.to_string()))?;
        let scale = 1.0 / f64::from(source.units_per_em());
        let underline = source.underline_metrics();
        let metrics = FontMetrics {
            ascender: (scale * f64::from(source.ascender())) as f32,
            descender: (scale * f64::from(source.descender())) as f32,
            line_height: (scale
                * (f64::from(source.ascender()) - f64::from(source.descender())
                    + f64::from(source.line_gap()))) as f32,
            underline_y: (scale * underline.map_or(0.0, |m| f64::from(m.position))) as f32,
            underline_thickness: (scale * underline.map_or(0.0, |m| f64::from(m.thickness))) as f32,
        };
        let mut glyphs = Vec::with_capacity(characters.len());
        let mut fields = Vec::with_capacity(characters.len());
        let mut x = 1;
        let mut y = 1;
        let mut row_height = 0;
        for character in characters {
            let glyph_id = source
                .glyph_index(character)
                .ok_or(FontError::MissingGlyph(character))?;
            let advance = (scale
                * f64::from(
                    source
                        .glyph_hor_advance(glyph_id)
                        .ok_or_else(|| FontError::Source("Missing horizontal advance".into()))?,
                )) as f32;
            let mut shape = Shape::new();
            let mut glyph = Glyph {
                character,
                glyph_id: glyph_id.0,
                advance,
                plane_bounds: [0.0; 4],
                atlas_bounds: [0; 4],
            };
            let outlined =
                source.outline_glyph(glyph_id, &mut ShapeBuilder::new(&mut shape, scale));
            if outlined.is_some() && shape.edge_count() > 0 {
                let (plane, w, h, pixels) = generate(&mut shape, config)?;
                if w + 2 > config.atlas_width {
                    return Err(FontError::Limit);
                }
                if x + w + 1 > config.atlas_width {
                    x = 1;
                    y += row_height + 1;
                    row_height = 0;
                }
                if y + h + 1 > config.max_atlas_height {
                    return Err(FontError::Limit);
                }
                glyph.plane_bounds = plane;
                glyph.atlas_bounds = [x, y, w, h];
                fields.push(pixels);
                x += w + 1;
                row_height = row_height.max(h);
            } else {
                fields.push(Vec::new());
            }
            glyphs.push(glyph);
        }
        let height = (y + row_height + 1).max(2);
        let mut pixels = vec![0; config.atlas_width as usize * height as usize * 4];
        for alpha in pixels.iter_mut().skip(3).step_by(4) {
            *alpha = 255;
        }
        for (g, field) in glyphs.iter().zip(fields) {
            let [x, y, w, h] = g.atlas_bounds;
            for row in 0..h as usize {
                let dst = ((y as usize + row) * config.atlas_width as usize + x as usize) * 4;
                pixels[dst..dst + w as usize * 4]
                    .copy_from_slice(&field[row * w as usize * 4..(row + 1) * w as usize * 4]);
            }
        }
        Font::new(FontData {
            width: config.atlas_width,
            height,
            pixels,
            kind: config.kind,
            distance_range: config.distance_range,
            pixels_per_em: config.pixels_per_em,
            metrics,
            glyphs,
        })
    }

    pub fn import_file(
        &self,
        source: &Path,
        output: &Path,
        config: &ImportConfig,
    ) -> Result<(), FontError> {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(source)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        let font = self.import_bytes(&bytes, config)?;
        std::fs::write(output, font.to_bytes()?)?;
        Ok(())
    }
}

#[arcana::importer]
impl Importer for FontImporter {
    fn desc() -> ImporterDesc {
        let fields = [
            ("characters", Model::String),
            (
                "kind",
                Model::Enum(vec![
                    (arcana::ident!(Sdf), None),
                    (arcana::ident!(Msdf), None),
                ]),
            ),
            ("pixels_per_em", Model::Float),
            ("distance_range", Model::Float),
            ("atlas_width", Model::Int),
            ("max_atlas_height", Model::Int),
            ("face_index", Model::Int),
        ];
        let model = Model::Record(
            fields
                .into_iter()
                .map(|(name, model)| {
                    (
                        arcana::Ident::from_static(name)
                            .expect("static configuration field is an identifier"),
                        Some(model),
                    )
                })
                .collect(),
        );
        let defaults = ImportConfig::default();
        let value = Value::Map(
            [
                (
                    "characters".into(),
                    Value::String(defaults.characters.into()),
                ),
                (
                    "kind".into(),
                    Value::Enum(arcana::ident!(Msdf), Box::new(Value::Unit)),
                ),
                (
                    "pixels_per_em".into(),
                    Value::Float(defaults.pixels_per_em.into()),
                ),
                (
                    "distance_range".into(),
                    Value::Float(defaults.distance_range.into()),
                ),
                (
                    "atlas_width".into(),
                    Value::Int(defaults.atlas_width.into()),
                ),
                (
                    "max_atlas_height".into(),
                    Value::Int(defaults.max_atlas_height.into()),
                ),
                ("face_index".into(), Value::Int(defaults.face_index.into())),
            ]
            .into_iter()
            .collect(),
        );
        ImporterDesc {
            name: arcana::format_name!("MSDF font"),
            formats: vec![arcana::name!(TrueType)],
            extensions: vec!["ttf"],
            target: arcana::ident!(msdf_font),
            config: (model, value),
        }
    }
    fn import(
        &self,
        source: &Path,
        output: &Path,
        config: Value,
        _: &mut dyn ImportContext,
    ) -> Result<(), ImportError> {
        let config = ImportConfig::deserialize(config).map_err(|e| ImportError::Other {
            reason: e.to_string(),
        })?;
        self.import_file(source, output, &config)
            .map_err(|e| ImportError::Other {
                reason: e.to_string(),
            })
    }
}

struct ShapeBuilder<'a> {
    shape: &'a mut Shape,
    scale: f64,
    position: Vector2,
    start: Vector2,
}

impl<'a> ShapeBuilder<'a> {
    fn new(shape: &'a mut Shape, scale: f64) -> Self {
        Self {
            shape,
            scale,
            position: Vector2::ZERO,
            start: Vector2::ZERO,
        }
    }

    fn point(&self, x: f32, y: f32) -> Vector2 {
        Vector2::new(self.scale * f64::from(x), self.scale * f64::from(y))
    }

    fn add_edge(&mut self, edge: EdgeSegment) {
        self.shape
            .contours
            .last_mut()
            .expect("outline segments follow move_to")
            .add_edge(edge);
    }
}

impl OutlineBuilder for ShapeBuilder<'_> {
    fn move_to(&mut self, x: f32, y: f32) {
        self.shape.add_contour(Contour::new());
        self.position = self.point(x, y);
        self.start = self.position;
    }

    fn line_to(&mut self, x: f32, y: f32) {
        let end = self.point(x, y);
        if end != self.position {
            self.add_edge(EdgeSegment::line(self.position, end));
        }
        self.position = end;
    }

    fn quad_to(&mut self, x1: f32, y1: f32, x: f32, y: f32) {
        let control = self.point(x1, y1);
        let end = self.point(x, y);
        if end != self.position {
            self.add_edge(EdgeSegment::quadratic(self.position, control, end));
        } else if control != self.position {
            // The simplifying constructor collapses closed curves to a zero-length line.
            self.add_edge(EdgeSegment::Quadratic([self.position, control, end]));
        }
        self.position = end;
    }

    fn curve_to(&mut self, x1: f32, y1: f32, x2: f32, y2: f32, x: f32, y: f32) {
        let c1 = self.point(x1, y1);
        let c2 = self.point(x2, y2);
        let end = self.point(x, y);
        if end != self.position {
            self.add_edge(EdgeSegment::cubic(self.position, c1, c2, end));
        } else if c1 != self.position || c2 != self.position {
            self.add_edge(EdgeSegment::Cubic([self.position, c1, c2, end]));
        }
        self.position = end;
    }

    fn close(&mut self) {
        if self.position != self.start {
            self.add_edge(EdgeSegment::line(self.position, self.start));
            self.position = self.start;
        }
    }
}

fn generate(
    shape: &mut Shape,
    config: &ImportConfig,
) -> Result<([f32; 4], u32, u32, Vec<u8>), FontError> {
    if shape.edge_count() > 65536 || !shape.validate() {
        return Err(FontError::Invalid("outline"));
    }
    shape.normalize();
    shape.orient_contours();
    let b = shape.get_bounds(0.0);
    let scale = config.pixels_per_em as f64;
    let pad = (config.distance_range as f64 * 0.5).ceil() + 1.0;
    let l = (b.l * scale).floor() - pad;
    let bottom = (b.b * scale).floor() - pad;
    let r = (b.r * scale).ceil() + pad;
    let top = (b.t * scale).ceil() + pad;
    if [l, bottom, r, top]
        .iter()
        .any(|v| !v.is_finite() || v.abs() > 524288.0)
        || r <= l
        || top <= bottom
        || r - l > MAX_DIM as f64
        || top - bottom > MAX_DIM as f64
    {
        return Err(FontError::Limit);
    }
    let w = (r - l) as u32;
    let h = (top - bottom) as u32;
    if w + 2 > config.atlas_width || h + 2 > config.max_atlas_height {
        return Err(FontError::Limit);
    }
    let transform = SdfTransformation::new(
        Projection::new(
            Vector2::splat(scale),
            Vector2::new(-l / scale, -bottom / scale),
        ),
        DistanceMapping::from_range(Range::symmetric(config.distance_range as f64 / scale)),
    );
    let mut pixels = vec![0; w as usize * h as usize * 4];
    let mut store = |x: usize, y: usize, rgb: &[f32]| -> Result<(), FontError> {
        if rgb.iter().any(|v| !v.is_finite()) {
            return Err(FontError::Invalid("non-finite generated distance"));
        }
        let dst = ((h as usize - 1 - y) * w as usize + x) * 4;
        for c in 0..3 {
            pixels[dst + c] = (rgb[c % rgb.len()].clamp(0.0, 1.0) * 255.0).round() as u8;
        }
        pixels[dst + 3] = 255;
        Ok(())
    };
    match config.kind {
        FieldKind::Msdf => {
            bymsdfgen_core::coloring::edge_coloring_simple(shape, 3.0, 0);
            let mut bitmap = Bitmap::<f32, 3>::new(w as usize, h as usize);
            generate_msdf(&mut bitmap, shape, &transform, &Default::default());
            for y in 0..h as usize {
                for x in 0..w as usize {
                    store(x, y, bitmap.pixel(x, y))?;
                }
            }
        }
        FieldKind::Sdf => {
            let mut bitmap = Bitmap::<f32, 1>::new(w as usize, h as usize);
            generate_sdf(&mut bitmap, shape, &transform, &Default::default());
            for y in 0..h as usize {
                for x in 0..w as usize {
                    store(x, y, bitmap.pixel(x, y))?;
                }
            }
        }
    }
    Ok((
        [
            (l / scale) as f32,
            (bottom / scale) as f32,
            (r / scale) as f32,
            (top / scale) as f32,
        ],
        w,
        h,
        pixels,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn outline_curves_preserve_coordinates_and_close() {
        let mut shape = Shape::new();
        let scale = 1.0 / 1000.0;
        let mut builder = ShapeBuilder::new(&mut shape, scale);
        builder.move_to(123.0, 17.0);
        builder.quad_to(100.0, 800.0, 500.0, 700.0);
        builder.curve_to(600.0, 600.0, 900.0, 100.0, 800.0, 0.0);
        builder.close();
        builder.close();
        assert!(shape.validate());
        assert!(!shape.inverse_y_axis);
        let segments = &shape.contours[0].segments;
        assert_eq!(segments.len(), 3);
        let point = |x, y| Vector2::new(scale * x, scale * y);
        assert_eq!(
            segments[0],
            EdgeSegment::Quadratic([point(123.0, 17.0), point(100.0, 800.0), point(500.0, 700.0),])
        );
        assert_eq!(
            segments[1],
            EdgeSegment::Cubic([
                point(500.0, 700.0),
                point(600.0, 600.0),
                point(900.0, 100.0),
                point(800.0, 0.0),
            ])
        );
        assert_eq!(
            segments[2],
            EdgeSegment::Line([point(800.0, 0.0), point(123.0, 17.0)])
        );
    }

    #[test]
    fn closed_curves_survive_and_constant_segments_are_skipped() {
        let mut shape = Shape::new();
        let mut builder = ShapeBuilder::new(&mut shape, 1.0);
        builder.move_to(0.0, 0.0);
        builder.line_to(0.0, 0.0);
        builder.quad_to(0.0, 0.0, 0.0, 0.0);
        builder.curve_to(0.0, 0.0, 0.0, 0.0, 0.0, 0.0);
        builder.quad_to(2.0, 0.0, 0.0, 0.0);
        builder.close();
        builder.move_to(0.0, 0.0);
        builder.curve_to(2.0, 0.0, 2.0, 0.0, 0.0, 0.0);
        builder.close();
        assert!(shape.validate());
        assert_eq!(shape.contours.len(), 2);
        assert_eq!(shape.edge_count(), 2);
        let quadratic = shape.contours[0].segments[0];
        let cubic = shape.contours[1].segments[0];
        assert!(matches!(quadratic, EdgeSegment::Quadratic(_)));
        assert!(matches!(cubic, EdgeSegment::Cubic(_)));
        assert_eq!(quadratic.point(0.5), Vector2::new(1.0, 0.0));
        assert_eq!(cubic.point(0.5), Vector2::new(1.5, 0.0));
    }
}
