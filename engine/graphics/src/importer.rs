use std::{fs::File, io::BufWriter, path::Path};

use arcana_assets::import::{ImportContext, ImportError, Importer, ImporterDesc, ImporterId};
use arcana_intern::{Ident, Name};
use arcana_model::{Model, Value, serialize_to_value};
use image::ImageFormat;
use jkl::{
    image::{Image, block::bc1},
    jackal::image::{Compression, Options, write_image},
    math::{R8U, Rg8U, Rgb8U, Rgba8U},
};
use serde::{Deserialize, Serialize};

/// Storage formats supported by the JKLI texture loader.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImageTargetFormat {
    #[serde(rename = "R8")]
    R8,
    #[serde(rename = "RG8")]
    Rg8,
    #[default]
    #[serde(rename = "RGBA8")]
    Rgba8,
    #[serde(rename = "RGB8")]
    Rgb8,
    #[serde(rename = "BC1")]
    Bc1,
}

/// A source channel or an eight-bit constant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Channel {
    R,
    G,
    B,
    A,
    Zero,
    One,
}

impl Channel {
    fn select(self, source: [u8; 4]) -> u8 {
        match self {
            Self::R => source[0],
            Self::G => source[1],
            Self::B => source[2],
            Self::A => source[3],
            Self::Zero => 0,
            Self::One => 255,
        }
    }
}

/// Destination channels selected simultaneously from the decoded source RGBA.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ChannelSwizzle {
    pub red: Channel,
    pub green: Channel,
    pub blue: Channel,
    pub alpha: Channel,
}

impl Default for ChannelSwizzle {
    fn default() -> Self {
        Self {
            red: Channel::R,
            green: Channel::G,
            blue: Channel::B,
            alpha: Channel::A,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ImageImportConfig {
    pub format: ImageTargetFormat,
    pub swizzle: ChannelSwizzle,
    /// BC1 texels with swizzled alpha below this normalized threshold are transparent.
    /// Equality is opaque. Must be finite and in 0..=1; ignored for uncompressed formats.
    pub alpha_cutoff: f32,
}

impl Default for ImageImportConfig {
    fn default() -> Self {
        Self {
            format: ImageTargetFormat::Rgba8,
            swizzle: ChannelSwizzle::default(),
            alpha_cutoff: 0.5,
        }
    }
}

/// Imports the first image/frame as a single-level JKLI texture.
/// Samples convert to eight bits per channel; alpha remains unpremultiplied.
#[derive(Default)]
pub struct ImageImporter;

impl ImageImporter {
    pub fn id() -> ImporterId {
        arcana_id::hash_id!("arcana::graphics::ImageImporter" => ImporterId)
    }
}

impl Importer for ImageImporter {
    fn desc() -> ImporterDesc {
        ImporterDesc {
            name: Name::from_name_static("Image"),
            formats: ["PNG", "JPEG", "TGA", "TIFF", "BMP", "GIF", "QOI", "WebP"]
                .into_iter()
                .map(Name::from_name_static)
                .collect(),
            extensions: vec![
                "png", "jpg", "jpeg", "tga", "tif", "tiff", "bmp", "gif", "qoi", "webp",
            ],
            target: Ident::from_static("Texture").unwrap(),
            config: (
                Model::Record(vec![
                    (
                        Ident::from_static("format").unwrap(),
                        Some(enum_model(&["R8", "RG8", "RGB8", "RGBA8", "BC1"])),
                    ),
                    (
                        Ident::from_static("swizzle").unwrap(),
                        Some(Model::Record(
                            ["red", "green", "blue", "alpha"]
                                .into_iter()
                                .map(|name| {
                                    (
                                        Ident::from_static(name).unwrap(),
                                        Some(enum_model(&["R", "G", "B", "A", "Zero", "One"])),
                                    )
                                })
                                .collect(),
                        )),
                    ),
                    (
                        Ident::from_static("alpha_cutoff").unwrap(),
                        Some(Model::Float),
                    ),
                ]),
                serialize_to_value(&ImageImportConfig::default())
                    .expect("image configuration is serializable"),
            ),
        }
    }

    fn import(
        &self,
        source: &Path,
        output: &Path,
        config: Value,
        _context: &mut dyn ImportContext,
    ) -> Result<(), ImportError> {
        let error = |reason: String| ImportError::Other { reason };
        let config = if config == Value::Unit {
            ImageImportConfig::default()
        } else {
            ImageImportConfig::deserialize(config)
                .map_err(|err| error(format!("Invalid image configuration: {err}")))?
        };
        if !config.alpha_cutoff.is_finite() || !(0.0..=1.0).contains(&config.alpha_cutoff) {
            return Err(error(
                "Image alpha cutoff must be finite and in 0..=1".into(),
            ));
        }
        let reader = image::ImageReader::open(source)
            .and_then(|reader| reader.with_guessed_format())
            .map_err(|err| error(format!("Failed to open image: {err}")))?;
        if !matches!(
            reader.format(),
            Some(
                ImageFormat::Png
                    | ImageFormat::Jpeg
                    | ImageFormat::Tga
                    | ImageFormat::Tiff
                    | ImageFormat::Bmp
                    | ImageFormat::Gif
                    | ImageFormat::Qoi
                    | ImageFormat::WebP
            )
        ) {
            return Err(error("Unsupported image format".into()));
        }
        let image = reader
            .decode()
            .map_err(|err| error(format!("Failed to decode image: {err}")))?
            .into_rgba8();
        let (width, height) = (image.width() as usize, image.height() as usize);
        if config.format == ImageTargetFormat::Bc1 && (width % 4 != 0 || height % 4 != 0) {
            return Err(error(
                "BC1 image dimensions must be multiples of four".into(),
            ));
        }
        let pixels = image
            .pixels()
            .map(|pixel| {
                Rgba8U([
                    config.swizzle.red.select(pixel.0),
                    config.swizzle.green.select(pixel.0),
                    config.swizzle.blue.select(pixel.0),
                    config.swizzle.alpha.select(pixel.0),
                ])
            })
            .collect::<Vec<_>>();
        let image = Image::new_2d(width, height, pixels.into_boxed_slice());
        let mut output = BufWriter::new(
            File::create(output)
                .map_err(|err| error(format!("Failed to create texture: {err}")))?,
        );
        let options = Options::new().with_compression(Compression::Ans);
        match config.format {
            ImageTargetFormat::R8 => {
                let pixels = image
                    .as_ref()
                    .iter_pixels()
                    .map(|pixel| R8U(pixel.0[0]))
                    .collect::<Vec<_>>();
                write_image(
                    Image::new_2d(width, height, pixels.into_boxed_slice()).as_ref(),
                    options,
                    &mut output,
                )
            }
            ImageTargetFormat::Rg8 => {
                let pixels = image
                    .as_ref()
                    .iter_pixels()
                    .map(|pixel| Rg8U([pixel.0[0], pixel.0[1]]))
                    .collect::<Vec<_>>();
                write_image(
                    Image::new_2d(width, height, pixels.into_boxed_slice()).as_ref(),
                    options,
                    &mut output,
                )
            }
            ImageTargetFormat::Rgba8 => write_image(image.as_ref(), options, &mut output),
            ImageTargetFormat::Rgb8 => {
                let pixels = image
                    .as_ref()
                    .iter_pixels()
                    .map(|pixel| Rgb8U([pixel.0[0], pixel.0[1], pixel.0[2]]))
                    .collect::<Vec<_>>();
                write_image(
                    Image::new_2d(width, height, pixels.into_boxed_slice()).as_ref(),
                    options,
                    &mut output,
                )
            }
            ImageTargetFormat::Bc1 => {
                let mut blocks = Image::new_2d(
                    width / 4,
                    height / 4,
                    vec![bc1::Block::BLACK; width * height / 16].into_boxed_slice(),
                );
                bc1::encode_image_with_alpha(
                    image.as_ref(),
                    Rgba8U::into_f32,
                    config.alpha_cutoff,
                    blocks.as_mut(),
                );
                write_image(blocks.as_ref(), options, &mut output)
            }
        }
        .map_err(|err| error(format!("Failed to encode texture: {err}")))?;
        std::io::Write::flush(&mut output)
            .map_err(|err| error(format!("Failed to write texture: {err}")))
    }
}

fn enum_model(variants: &[&'static str]) -> Model {
    Model::Enum(
        variants
            .iter()
            .map(|name| (Ident::from_static(name).unwrap(), Some(Model::Unit)))
            .collect(),
    )
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use arcana_assets::{Asset, Assets};
    use jkl::{
        image::{Extent, Image2DMut, format::Format},
        jackal::image::JackalImageReader,
    };
    use std::{
        io::Cursor,
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct NoDependencies;
    impl ImportContext for NoDependencies {
        fn get_dependency(
            &mut self,
            _: &str,
            _: Ident,
        ) -> Result<arcana_assets::AssetId, arcana_assets::import::Missing> {
            panic!("Image importer requested dependency");
        }
        fn get_source(&mut self, _: &str) -> Result<PathBuf, arcana_assets::import::Missing> {
            panic!("Image importer requested source");
        }
    }

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "arcana-image-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn import(&self, config: Value) -> Result<(), ImportError> {
            ImageImporter.import(
                &self.0.join("source.png"),
                &self.0.join("output.jkli"),
                config,
                &mut NoDependencies,
            )
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn imported(pixels: &[u8], width: u32, height: u32, config: &ImageImportConfig) -> Box<[u8]> {
        let fixture = Fixture::new();
        image::save_buffer(
            fixture.0.join("source.png"),
            pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .unwrap();
        assert!(fixture.import(serialize_to_value(config).unwrap()).is_ok());
        std::fs::read(fixture.0.join("output.jkli"))
            .unwrap()
            .into_boxed_slice()
    }

    fn decode<T: jkl::jackal::image::Pixel>(bytes: &[u8], initial: T) -> (usize, usize, Vec<T>) {
        let mut reader = JackalImageReader::open(Cursor::new(bytes)).unwrap();
        let Extent::D2 { width, height } = reader.extent() else {
            panic!("2D fixture")
        };
        let mut pixels = vec![initial; width * height];
        let mut image = Image2DMut::new(width, height, &mut pixels[..]);
        let mut reader = reader.tile_reader::<T>().unwrap();
        for index in 0..reader.tiles() {
            let tile = reader.tile(index);
            reader
                .read_tile(index, image.get_rect_mut(tile.rect))
                .unwrap();
        }
        (width, height, pixels)
    }

    #[test]
    fn configuration_model_defaults_and_serde_roundtrip() {
        let desc = ImageImporter::desc();
        assert!(matches!(desc.config.0, Model::Record(_)));
        assert_eq!(
            ImageImportConfig::deserialize(desc.config.1.clone()).unwrap(),
            ImageImportConfig::default()
        );
        let empty = Value::Map(Default::default());
        assert_eq!(
            ImageImportConfig::deserialize(empty).unwrap(),
            ImageImportConfig::default()
        );
        for format in [
            ImageTargetFormat::R8,
            ImageTargetFormat::Rg8,
            ImageTargetFormat::Rgba8,
            ImageTargetFormat::Rgb8,
            ImageTargetFormat::Bc1,
        ] {
            let config = ImageImportConfig {
                format,
                swizzle: ChannelSwizzle {
                    red: Channel::A,
                    green: Channel::Zero,
                    blue: Channel::One,
                    alpha: Channel::R,
                },
                alpha_cutoff: 0.25,
            };
            assert_eq!(
                ImageImportConfig::deserialize(serialize_to_value(&config).unwrap()).unwrap(),
                config
            );
        }
        let fixture = Fixture::new();
        image::save_buffer(
            fixture.0.join("source.png"),
            &[11, 22, 33, 44],
            1,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        assert!(fixture.import(Value::Unit).is_ok());
        let legacy = std::fs::read(fixture.0.join("output.jkli")).unwrap();
        assert!(fixture.import(desc.config.1).is_ok());
        assert_eq!(
            std::fs::read(fixture.0.join("output.jkli")).unwrap(),
            legacy
        );
    }

    #[test]
    fn swizzles_read_original_channels_and_constants_before_conversion() {
        let pixels = [12, 34, 56, 78, 90, 100, 110, 120];
        let mut config = ImageImportConfig {
            swizzle: ChannelSwizzle {
                red: Channel::B,
                green: Channel::R,
                blue: Channel::A,
                alpha: Channel::G,
            },
            ..Default::default()
        };
        assert_eq!(
            decode::<Rgba8U>(&imported(&pixels, 2, 1, &config), Rgba8U([0; 4])).2,
            [Rgba8U([56, 12, 78, 34]), Rgba8U([110, 90, 120, 100])]
        );
        config.format = ImageTargetFormat::Rgb8;
        assert_eq!(
            decode::<Rgb8U>(&imported(&pixels, 2, 1, &config), Rgb8U([0; 3])).2,
            [Rgb8U([56, 12, 78]), Rgb8U([110, 90, 120])]
        );
        config.format = ImageTargetFormat::Rgba8;
        config.swizzle = ChannelSwizzle {
            red: Channel::Zero,
            green: Channel::One,
            blue: Channel::G,
            alpha: Channel::One,
        };
        assert_eq!(
            decode::<Rgba8U>(&imported(&pixels, 2, 1, &config), Rgba8U([0; 4])).2,
            [Rgba8U([0, 255, 34, 255]), Rgba8U([0, 255, 100, 255])]
        );
        config.swizzle = ChannelSwizzle::default();
        for format in [ImageTargetFormat::Rgba8, ImageTargetFormat::Rgb8] {
            config.format = format;
            config.alpha_cutoff = 0.0;
            let zero = imported(&pixels, 2, 1, &config);
            config.alpha_cutoff = 1.0;
            assert_eq!(imported(&pixels, 2, 1, &config), zero);
        }
    }

    #[test]
    fn one_and_two_channel_outputs_follow_swizzle() {
        let pixels = [12, 34, 56, 78, 90, 100, 110, 120, 1, 2, 3, 4];
        let mut config = ImageImportConfig {
            format: ImageTargetFormat::R8,
            swizzle: ChannelSwizzle {
                red: Channel::A,
                green: Channel::B,
                blue: Channel::One,
                alpha: Channel::Zero,
            },
            ..Default::default()
        };
        assert_eq!(
            decode::<R8U>(&imported(&pixels, 3, 1, &config), R8U(0)).2,
            [R8U(78), R8U(120), R8U(4)]
        );
        config.format = ImageTargetFormat::Rg8;
        assert_eq!(
            decode::<Rg8U>(&imported(&pixels, 3, 1, &config), Rg8U([0; 2])).2,
            [Rg8U([78, 56]), Rg8U([120, 110]), Rg8U([4, 3])]
        );
        config.swizzle.red = Channel::Zero;
        config.swizzle.green = Channel::One;
        assert_eq!(
            decode::<Rg8U>(&imported(&pixels, 3, 1, &config), Rg8U([0; 2])).2,
            [Rg8U([0, 255]); 3]
        );
        config.format = ImageTargetFormat::R8;
        config.swizzle.red = Channel::One;
        assert_eq!(
            decode::<R8U>(&imported(&pixels, 3, 1, &config), R8U(0)).2,
            [R8U(255); 3]
        );
        for format in [ImageTargetFormat::R8, ImageTargetFormat::Rg8] {
            config.format = format;
            config.alpha_cutoff = 0.0;
            let zero = imported(&pixels, 3, 1, &config);
            config.alpha_cutoff = 1.0;
            assert_eq!(imported(&pixels, 3, 1, &config), zero);
        }
    }

    #[test]
    fn bc1_alpha_cutoff_endpoints_equality_and_swizzled_alpha() {
        let mut config = ImageImportConfig {
            format: ImageTargetFormat::Bc1,
            swizzle: ChannelSwizzle {
                red: Channel::One,
                green: Channel::Zero,
                blue: Channel::Zero,
                alpha: Channel::R,
            },
            ..Default::default()
        };
        let pixels = (0..16)
            .flat_map(|i| {
                [
                    if i % 4 == 0 {
                        0
                    } else if i % 4 == 1 {
                        127
                    } else if i % 4 == 2 {
                        128
                    } else {
                        255
                    },
                    32,
                    64,
                    255,
                ]
            })
            .collect::<Vec<_>>();
        for cutoff in [0.0, 128.0 / 255.0, 1.0] {
            config.alpha_cutoff = cutoff;
            let (_, _, blocks) =
                decode::<bc1::Block>(&imported(&pixels, 4, 4, &config), bc1::Block::BLACK);
            for (index, pixel) in blocks[0]
                .decode_with_alpha()
                .into_iter()
                .flatten()
                .enumerate()
            {
                let expected_alpha = if pixels[index * 4] as f32 / 255.0 >= cutoff {
                    255
                } else {
                    0
                };
                let actual = Rgba8U::from_f32(pixel).0;
                assert_eq!(actual[3], expected_alpha);
                if expected_alpha == 255 {
                    assert!(actual[0].abs_diff(255) <= 8);
                    assert!(actual[1] <= 8 && actual[2] <= 8);
                }
            }
        }
        config.swizzle.alpha = Channel::Zero;
        let (_, _, blocks) =
            decode::<bc1::Block>(&imported(&pixels, 4, 4, &config), bc1::Block::BLACK);
        assert!(
            blocks[0]
                .decode_with_alpha()
                .iter()
                .flatten()
                .all(|pixel| pixel.0[3] == 0.0)
        );
    }

    #[test]
    fn invalid_configuration_and_unaligned_bc1_never_create_output() {
        let fixture = Fixture::new();
        image::save_buffer(
            fixture.0.join("source.png"),
            &[1, 2, 3, 4],
            1,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        for cutoff in [-0.1, 1.1, f32::NAN, f32::INFINITY] {
            let config = ImageImportConfig {
                alpha_cutoff: cutoff,
                ..Default::default()
            };
            assert!(
                fixture
                    .import(serialize_to_value(&config).unwrap())
                    .is_err()
            );
            assert!(!fixture.0.join("output.jkli").exists());
        }
        let mut invalid = ImageImporter::desc().config.1;
        if let Value::Map(fields) = &mut invalid {
            fields.insert(
                "format".into(),
                Value::Enum(Ident::from_static("BC7").unwrap(), Box::new(Value::Unit)),
            );
        }
        assert!(fixture.import(invalid).is_err());
        let mut invalid = ImageImporter::desc().config.1;
        if let Value::Map(fields) = &mut invalid {
            fields.insert("unknown".into(), Value::Unit);
        }
        assert!(fixture.import(invalid).is_err());
        let mut invalid = ImageImporter::desc().config.1;
        if let Value::Map(fields) = &mut invalid {
            if let Value::Map(swizzle) = fields.get_mut("swizzle").unwrap() {
                swizzle.insert(
                    "red".into(),
                    Value::Enum(
                        Ident::from_static("Invalid").unwrap(),
                        Box::new(Value::Unit),
                    ),
                );
            }
        }
        assert!(fixture.import(invalid).is_err());
        let mut invalid = ImageImporter::desc().config.1;
        if let Value::Map(fields) = &mut invalid {
            fields.insert("alpha_cutoff".into(), Value::String("half".into()));
        }
        assert!(fixture.import(invalid).is_err());
        let config = ImageImportConfig {
            format: ImageTargetFormat::Bc1,
            ..Default::default()
        };
        assert!(
            fixture
                .import(serialize_to_value(&config).unwrap())
                .is_err()
        );
        assert!(!fixture.0.join("output.jkli").exists());
    }

    pub(crate) fn configured_gpu_fixtures() -> Vec<(Box<[u8]>, (u32, u32), Vec<u8>)> {
        let (width, height) = (132, 68);
        let pixels = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    [
                        ((x / 4) % 2 * 255) as u8,
                        ((y / 4) % 2 * 255) as u8,
                        ((x + y) % 256) as u8,
                        255,
                    ]
                })
            })
            .collect::<Vec<_>>();
        let mut config = ImageImportConfig {
            swizzle: ChannelSwizzle {
                red: Channel::G,
                green: Channel::R,
                blue: Channel::One,
                alpha: Channel::B,
            },
            ..Default::default()
        };
        let rgba = imported(&pixels, width, height, &config);
        let expected = decode::<Rgba8U>(&rgba, Rgba8U([0; 4]))
            .2
            .into_iter()
            .flat_map(|pixel| pixel.0)
            .collect();
        let mut fixtures = vec![(rgba, (width, height), expected)];
        config.format = ImageTargetFormat::Rgb8;
        let rgb = imported(&pixels, width, height, &config);
        let expected = decode::<Rgb8U>(&rgb, Rgb8U([0; 3]))
            .2
            .into_iter()
            .flat_map(|pixel| [pixel.0[0], pixel.0[1], pixel.0[2], 255])
            .collect();
        fixtures.push((rgb, (width, height), expected));
        config.format = ImageTargetFormat::Bc1;
        config.swizzle.alpha = Channel::R;
        let bc = imported(&pixels, width, height, &config);
        let (bw, bh, blocks) = decode::<bc1::Block>(&bc, bc1::Block::BLACK);
        let mut expected = vec![Rgba8U([0; 4]); width as usize * height as usize];
        bc1::decode_image(
            Image::new_2d(bw, bh, blocks.as_slice()).as_ref(),
            Rgba8U::from_f32,
            Image::new_2d(width as usize, height as usize, expected.as_mut_slice()).as_mut(),
        );
        fixtures.push((
            bc,
            (width, height),
            expected.into_iter().flat_map(|pixel| pixel.0).collect(),
        ));
        for (width, height) in [(133, 69), (1, 1), (2, 3), (3, 2), (4, 1)] {
            let pixels = (0..height)
                .flat_map(|y| {
                    (0..width).flat_map(move |x| {
                        [
                            ((x * 17 + y * 29) % 256) as u8,
                            ((x * 31 + y * 7) % 256) as u8,
                            ((x * 11 + y * 23) % 256) as u8,
                            ((x * 13 + y * 19) % 256) as u8,
                        ]
                    })
                })
                .collect::<Vec<_>>();
            config.swizzle = ChannelSwizzle {
                red: Channel::A,
                green: Channel::B,
                blue: Channel::One,
                alpha: Channel::Zero,
            };
            for format in [ImageTargetFormat::R8, ImageTargetFormat::Rg8] {
                config.format = format;
                let data = imported(&pixels, width, height, &config);
                let expected = pixels
                    .chunks_exact(4)
                    .flat_map(|pixel| {
                        [
                            pixel[3],
                            if format == ImageTargetFormat::Rg8 {
                                pixel[2]
                            } else {
                                0
                            },
                            0,
                            255,
                        ]
                    })
                    .collect();
                fixtures.push((data, (width, height), expected));
            }
        }
        fixtures
    }

    pub(crate) fn imported_png() -> (Box<[u8]>, (u32, u32), Vec<u8>) {
        let fixture = Fixture::new();
        let (width, height) = (67, 35);
        let pixels = (0..height)
            .flat_map(|y| {
                (0..width).flat_map(move |x| {
                    [
                        x as u8 * 3,
                        y as u8 * 7,
                        (x ^ y) as u8,
                        ((x * 17 + y * 11) % 256) as u8,
                    ]
                })
            })
            .collect::<Vec<_>>();
        image::save_buffer(
            fixture.0.join("source.png"),
            &pixels,
            width,
            height,
            image::ColorType::Rgba8,
        )
        .unwrap();
        assert!(fixture.import(Value::Unit).is_ok());
        (
            std::fs::read(fixture.0.join("output.jkli"))
                .unwrap()
                .into_boxed_slice(),
            (width, height),
            pixels,
        )
    }

    #[test]
    fn png_roundtrip_preserves_dimensions_colors_and_alpha() {
        let (bytes, (width, height), expected) = imported_png();
        let mut reader = JackalImageReader::open(Cursor::new(&bytes[..])).unwrap();
        assert_eq!(reader.format(), Format::RGBA8);
        assert_eq!(reader.compression(), Compression::Ans);
        assert_eq!(
            reader.extent(),
            Extent::D2 {
                width: width as usize,
                height: height as usize
            }
        );
        let mut pixels = vec![Rgba8U([0; 4]); (width * height) as usize];
        let mut image = Image2DMut::new(width as usize, height as usize, &mut pixels[..]);
        let mut reader = reader.tile_reader::<Rgba8U>().unwrap();
        for index in 0..reader.tiles() {
            let tile = reader.tile(index);
            reader
                .read_tile(index, image.get_rect_mut(tile.rect))
                .unwrap();
        }
        assert_eq!(
            pixels
                .into_iter()
                .flat_map(|pixel| pixel.0)
                .collect::<Vec<_>>(),
            expected
        );
        assert!(
            futures::executor::block_on(crate::texture::Texture::load(bytes, &Assets::new([])))
                .is_ok()
        );
    }

    #[test]
    fn malformed_source_and_nonunit_configuration_fail_without_output() {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("source.png"), b"invalid PNG").unwrap();
        for config in [Value::Unit, Value::Bool(false)] {
            assert!(matches!(
                fixture.import(config),
                Err(ImportError::Other { .. })
            ));
            assert!(!fixture.0.join("output.jkli").exists());
        }
        image::save_buffer(
            fixture.0.join("source.png"),
            &[255, 0, 0, 128],
            1,
            1,
            image::ColorType::Rgba8,
        )
        .unwrap();
        assert!(
            matches!(fixture.import(Value::Bool(false)), Err(ImportError::Other { reason }) if reason.contains("configuration"))
        );
        assert!(!fixture.0.join("output.jkli").exists());
    }

    #[test]
    fn tiny_jpeg_imports_as_opaque_rgba_texture() {
        let fixture = Fixture::new();
        let source = fixture.0.join("tiny.jpg");
        let output = fixture.0.join("tiny.jkli");
        image::save_buffer(&source, &[35, 80, 130], 1, 1, image::ColorType::Rgb8).unwrap();
        assert!(
            ImageImporter
                .import(&source, &output, Value::Unit, &mut NoDependencies)
                .is_ok()
        );
        let bytes = std::fs::read(output).unwrap().into_boxed_slice();
        let mut reader = JackalImageReader::open(Cursor::new(&bytes[..])).unwrap();
        assert_eq!(
            reader.extent(),
            Extent::D2 {
                width: 1,
                height: 1
            }
        );
        let mut pixels = [Rgba8U([0; 4])];
        reader
            .tile_reader::<Rgba8U>()
            .unwrap()
            .read_tile(0, Image2DMut::new(1, 1, &mut pixels[..]))
            .unwrap();
        assert_eq!(
            pixels[0].0,
            image::open(source).unwrap().into_rgba8().get_pixel(0, 0).0
        );
        assert_eq!(pixels[0].0[3], 255);
        assert!(
            futures::executor::block_on(crate::texture::Texture::load(bytes, &Assets::new([])))
                .is_ok()
        );
    }
}
