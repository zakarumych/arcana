//! Import TrueType outlines as reusable signed-distance font atlases.
mod asset;
mod gpu;
mod importer;

pub use asset::{FieldKind, Font, FontError, FontMetrics, Glyph, GlyphBitmap};
pub use gpu::RASTERIZER_WGSL;
pub use importer::{FontImporter, ImportConfig};

arcana::declare_plugin!();
