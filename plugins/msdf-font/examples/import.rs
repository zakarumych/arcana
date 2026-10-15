use arcana_msdf_font::{FieldKind, FontImporter, ImportConfig};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .ok_or("usage: import INPUT.ttf OUTPUT.arcfont [msdf|sdf] [PREVIEW.pgm]")?;
    let output = args.next().ok_or("missing output font path")?;
    let kind = match args.next().as_deref().and_then(|v| v.to_str()) {
        None | Some("msdf") => FieldKind::Msdf,
        Some("sdf") => FieldKind::Sdf,
        _ => return Err("mode must be msdf or sdf".into()),
    };
    let config = ImportConfig {
        kind,
        ..Default::default()
    };
    let font = FontImporter.import_bytes(&std::fs::read(input)?, &config)?;
    std::fs::write(output, font.to_bytes()?)?;
    if let Some(preview) = args.next() {
        let bitmap = font.rasterize_glyph('A', 128.0)?;
        let mut pgm = format!("P5\n{} {}\n255\n", bitmap.width, bitmap.height).into_bytes();
        pgm.extend_from_slice(&bitmap.pixels);
        std::fs::write(preview, pgm)?;
    }
    println!(
        "Imported {} glyphs into {}x{} {:?} atlas",
        font.glyphs().len(),
        font.width(),
        font.height(),
        font.kind()
    );
    Ok(())
}
