use arcana_msdf_font::{FieldKind, Font, FontError, FontImporter, ImportConfig};

// Authored test outlines and SFNT tables; no external font or font license required.
fn fixture() -> Vec<u8> {
    fn u16_at(v: &mut [u8], at: usize, n: u16) {
        v[at..at + 2].copy_from_slice(&n.to_be_bytes());
    }
    fn u32_at(v: &mut [u8], at: usize, n: u32) {
        v[at..at + 4].copy_from_slice(&n.to_be_bytes());
    }
    let mut head = vec![0; 54];
    u32_at(&mut head, 0, 0x10000);
    u32_at(&mut head, 12, 0x5f0f3cf5);
    u16_at(&mut head, 18, 1000);
    u16_at(&mut head, 50, 1);
    let mut hhea = vec![0; 36];
    u32_at(&mut hhea, 0, 0x10000);
    u16_at(&mut hhea, 4, 900);
    u16_at(&mut hhea, 6, (-200i16) as u16);
    u16_at(&mut hhea, 34, 3);
    let mut maxp = vec![0; 32];
    u32_at(&mut maxp, 0, 0x10000);
    u16_at(&mut maxp, 4, 3);
    let mut hmtx = vec![0; 12];
    for i in 0..3 {
        u16_at(&mut hmtx, i * 4, if i == 1 { 400 } else { 1000 });
    }
    // Glyph 2 is a square ring: sharp corners, clockwise outer, counterclockwise hole.
    let pts: [(i16, i16); 8] = [
        (0, 0),
        (0, 800),
        (800, 800),
        (800, 0),
        (250, 250),
        (550, 250),
        (550, 550),
        (250, 550),
    ];
    let mut glyf = vec![0; 10];
    u16_at(&mut glyf, 0, 2);
    u16_at(&mut glyf, 6, 800);
    u16_at(&mut glyf, 8, 800);
    glyf.extend_from_slice(&3u16.to_be_bytes());
    glyf.extend_from_slice(&7u16.to_be_bytes());
    glyf.extend_from_slice(&0u16.to_be_bytes());
    glyf.extend_from_slice(&[1; 8]);
    let mut prev = 0;
    for (x, _) in pts {
        glyf.extend_from_slice(&(x - prev).to_be_bytes());
        prev = x;
    }
    prev = 0;
    for (_, y) in pts {
        glyf.extend_from_slice(&(y - prev).to_be_bytes());
        prev = y;
    }
    let mut loca = vec![0; 16];
    u32_at(&mut loca, 12, glyf.len() as u32);
    // Format 12 cmap maps space to empty glyph 1, A to ring glyph 2.
    let mut cmap = vec![0; 52];
    u16_at(&mut cmap, 2, 1);
    u16_at(&mut cmap, 4, 3);
    u16_at(&mut cmap, 6, 10);
    u32_at(&mut cmap, 8, 12);
    u16_at(&mut cmap, 12, 12);
    u32_at(&mut cmap, 16, 40);
    u32_at(&mut cmap, 24, 2);
    for (i, (c, id)) in [(32, 1), (65, 2)].into_iter().enumerate() {
        let o = 28 + i * 12;
        u32_at(&mut cmap, o, c);
        u32_at(&mut cmap, o + 4, c);
        u32_at(&mut cmap, o + 8, id);
    }
    let tables = [
        (b"cmap", cmap),
        (b"glyf", glyf),
        (b"head", head),
        (b"hhea", hhea),
        (b"hmtx", hmtx),
        (b"loca", loca),
        (b"maxp", maxp),
    ];
    let mut bytes = vec![0; 12 + tables.len() * 16];
    u32_at(&mut bytes, 0, 0x10000);
    u16_at(&mut bytes, 4, tables.len() as u16);
    for (i, (tag, table)) in tables.into_iter().enumerate() {
        let offset = bytes.len();
        let row = 12 + i * 16;
        bytes[row..row + 4].copy_from_slice(tag);
        u32_at(&mut bytes, row + 8, offset as u32);
        u32_at(&mut bytes, row + 12, table.len() as u32);
        bytes.extend(table);
        while bytes.len() % 4 != 0 {
            bytes.push(0);
        }
    }
    bytes
}
fn font(kind: FieldKind) -> Font {
    FontImporter
        .import_bytes(
            &fixture(),
            &ImportConfig {
                characters: "A A".into(),
                kind,
                ..Default::default()
            },
        )
        .expect("test font imports")
}

#[test]
fn ttf_generation_roundtrip_and_determinism() {
    for kind in [FieldKind::Sdf, FieldKind::Msdf] {
        let f = font(kind);
        let bytes = f.to_bytes().expect("encode");
        assert_eq!(bytes, font(kind).to_bytes().expect("encode"));
        let decoded = Font::from_bytes(&bytes).expect("decode");
        assert_eq!(decoded.glyphs(), f.glyphs());
        assert_eq!(decoded.metrics(), f.metrics());
        assert_eq!(decoded.pixels(), f.pixels());
        assert_eq!(decoded.kind(), kind);
        assert_eq!(f.glyphs().len(), 2);
        assert_eq!(f.metrics().ascender, 0.9);
        assert_eq!(f.metrics().descender, -0.2);
        assert_eq!(f.metrics().line_height, 1.1);
        assert_eq!(f.metrics().underline_y, 0.0);
        assert_eq!(f.metrics().underline_thickness, 0.0);
        assert_eq!(f.glyphs()[1].glyph_id, 2);
        assert_eq!(f.glyphs()[1].advance, 1.0);
        let space = f.rasterize_glyph(' ', 48.0).expect("space");
        assert!(space.pixels.is_empty());
        assert_eq!(space.advance, 19.2);
    }
}

#[test]
fn signed_fields_have_sharp_channels_and_holes() {
    for kind in [FieldKind::Sdf, FieldKind::Msdf] {
        let f = font(kind);
        let bitmap = f.rasterize_glyph('A', 100.0).expect("raster");
        let coverage = |x: i32, y: i32| {
            bitmap.pixels[((y - bitmap.offset[1]) as u32 * bitmap.width
                + (x - bitmap.offset[0]) as u32) as usize]
        };
        assert_eq!(coverage(10, -10), 255, "inside ring");
        assert_eq!(coverage(40, -40), 0, "hole");
        assert_eq!(coverage(-3, -40), 0, "outside");
        let distinct = f
            .pixels()
            .chunks_exact(4)
            .any(|p| p[0] != p[1] || p[1] != p[2]);
        assert_eq!(distinct, kind == FieldKind::Msdf);
        let tiny = f.rasterize_glyph('A', 1.0).expect("minified");
        assert_eq!(tiny.pixels[0], 0, "saturated exterior stays transparent");
    }
}

#[test]
fn rejects_malformed_assets_and_missing_characters() {
    let bytes = font(FieldKind::Msdf).to_bytes().expect("encode");
    for len in 0..bytes.len() {
        assert!(Font::from_bytes(&bytes[..len]).is_err(), "prefix {len}");
    }
    for (offset, value) in [
        (8, 2),
        (12, 7),
        (16, u32::MAX),
        (24, f32::NAN.to_bits()),
        (52, u32::MAX),
        (56, 0xd800),
        (60, 65536),
        (64, f32::INFINITY.to_bits()),
        (84, u32::MAX),
        (100, ' ' as u32),
        (112, (-0.5f32).to_bits()),
        (128, u32::MAX),
        (136, 4096),
    ] {
        let mut bad = bytes.clone();
        bad[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        assert!(Font::from_bytes(&bad).is_err(), "offset {offset}");
    }
    let mut trailing = bytes;
    trailing.push(0);
    assert!(Font::from_bytes(&trailing).is_err());
    assert!(matches!(
        FontImporter.import_bytes(
            &fixture(),
            &ImportConfig {
                characters: "B".into(),
                ..Default::default()
            }
        ),
        Err(FontError::MissingGlyph('B'))
    ));
    assert!(
        FontImporter
            .import_bytes(&[], &ImportConfig::default())
            .is_err()
    );
    assert!(
        FontImporter
            .import_bytes(
                &fixture(),
                &ImportConfig {
                    face_index: 1,
                    characters: "A".into(),
                    ..Default::default()
                }
            )
            .is_err()
    );
}

#[test]
fn rejects_invalid_import_and_raster_limits() {
    for config in [
        ImportConfig {
            pixels_per_em: f32::NAN,
            ..Default::default()
        },
        ImportConfig {
            distance_range: 0.0,
            ..Default::default()
        },
        ImportConfig {
            atlas_width: u32::MAX,
            ..Default::default()
        },
        ImportConfig {
            characters: String::new(),
            ..Default::default()
        },
        ImportConfig {
            characters: "A".into(),
            atlas_width: 4,
            ..Default::default()
        },
    ] {
        assert!(FontImporter.import_bytes(&fixture(), &config).is_err());
    }
    let f = font(FieldKind::Msdf);
    for scale in [0.0, f32::NAN, f32::INFINITY, 4097.0] {
        assert!(f.rasterize_glyph('A', scale).is_err());
    }
    assert!(matches!(
        f.rasterize_glyph('B', 48.0),
        Err(FontError::MissingGlyph('B'))
    ));
}

#[test]
fn importer_registration_config_and_asset_load() {
    use arcana::assets::{Asset, Assets, import::Importer};
    use std::{
        future::Future,
        pin::pin,
        sync::Arc,
        task::{Context, Poll, Wake, Waker},
    };
    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let desc = FontImporter::desc();
    assert_eq!(desc.target.as_str(), "msdf_font");
    let config: ImportConfig =
        serde::Deserialize::deserialize(desc.config.1).expect("default config decodes");
    assert_eq!(config.characters, ImportConfig::default().characters);
    let bytes = font(FieldKind::Msdf).to_bytes().expect("encode");
    let assets = Assets::new([]);
    let mut future = pin!(Font::load(bytes.into_boxed_slice(), &assets));
    let waker = Waker::from(Arc::new(Noop));
    match future.as_mut().poll(&mut Context::from_waker(&waker)) {
        Poll::Ready(Ok(f)) => assert_eq!(f.glyphs().len(), 2),
        _ => panic!("font load must decode synchronously when polled"),
    };
}
