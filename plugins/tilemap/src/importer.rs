use std::{collections::HashSet, path::Path};

use arcana::{
    assets::{
        AssetId,
        import::{AssetDependency, ImportContext, ImportError, Importer, ImporterDesc},
    },
    model::{Model, Value},
};
use serde::{Deserialize, Serialize};

/// Imported tileset with a reference to an imported Texture asset.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TilesetData {
    pub image: AssetId,
    /// World-space width and height of a tile.
    pub tile_size: [f32; 2],
    pub tiles: Vec<TileData>,
}

/// A named rectangle in top-left atlas texel coordinates.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TileData {
    pub name: String,
    /// X, Y, width and height in texels.
    pub region: [u32; 4],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TilesetSource {
    image: String,
    tile_size: [f32; 2],
    tiles: Vec<TileData>,
}

/// Imported map referencing an imported tileset.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapData {
    pub tileset: AssetId,
    /// Bottom-to-top rows; zero is empty, positive values are 1-based tile indices.
    pub cells: Vec<Vec<u32>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapSource {
    tileset: String,
    cells: Vec<Vec<u32>>,
}

/// Imports JSON maps, requesting their tileset through the asset pipeline.
#[derive(Default)]
pub struct MapImporter;

#[arcana::importer]
impl Importer for MapImporter {
    fn desc() -> ImporterDesc {
        ImporterDesc {
            name: arcana::name!("Map importer"),
            formats: vec![arcana::name!("JSON map")],
            extensions: vec!["json"],
            target: arcana::ident!(Map),
            config: (Model::Unit, Value::Unit),
        }
    }

    fn import(
        &self,
        source: &Path,
        output: &Path,
        config: Value,
        context: &mut dyn ImportContext,
    ) -> Result<(), ImportError> {
        if !config.is_unit() {
            return Err(invalid("Map importer requires unit configuration"));
        }
        let bytes = std::fs::read(source).map_err(invalid)?;
        let source: MapSource = serde_json::from_slice(&bytes).map_err(invalid)?;
        if source.tileset.trim().is_empty() || source.tileset.chars().any(char::is_control) {
            return Err(invalid("Map tileset must be a nonempty source path"));
        }
        validate_cells(&source.cells).map_err(invalid)?;
        let target = arcana::ident!(Tileset);
        let tileset = context
            .get_dependency(&source.tileset, target)
            .map_err(|_| ImportError::Requires {
                sources: vec![],
                dependencies: vec![AssetDependency {
                    source: source.tileset.into(),
                    target,
                    format: None,
                }],
            })?;
        let artifact = MapData {
            tileset,
            cells: source.cells,
        };
        let bytes = serde_json::to_vec(&artifact).map_err(invalid)?;
        std::fs::write(output, bytes).map_err(invalid)
    }
}

/// Imports JSON tilesets, requesting their image through the asset pipeline.
#[derive(Default)]
pub struct TilesetImporter;

#[arcana::importer]
impl Importer for TilesetImporter {
    fn desc() -> ImporterDesc {
        ImporterDesc {
            name: arcana::name!("Tileset importer"),
            formats: vec![arcana::name!("JSON tileset")],
            extensions: vec!["json"],
            target: arcana::ident!(Tileset),
            config: (Model::Unit, Value::Unit),
        }
    }

    fn import(
        &self,
        source: &Path,
        output: &Path,
        config: Value,
        context: &mut dyn ImportContext,
    ) -> Result<(), ImportError> {
        if !config.is_unit() {
            return Err(invalid("Tileset importer requires unit configuration"));
        }
        let bytes = std::fs::read(source).map_err(invalid)?;
        let source: TilesetSource = serde_json::from_slice(&bytes).map_err(invalid)?;
        if source.image.trim().is_empty() || source.image.chars().any(char::is_control) {
            return Err(invalid("Tileset image must be a nonempty source path"));
        }
        validate_tiles(source.tile_size, &source.tiles).map_err(invalid)?;
        let target = arcana::ident!(Texture);
        let image =
            context
                .get_dependency(&source.image, target)
                .map_err(|_| ImportError::Requires {
                    sources: vec![],
                    dependencies: vec![AssetDependency {
                        source: source.image.into(),
                        target,
                        format: None,
                    }],
                })?;
        let artifact = TilesetData {
            image,
            tile_size: source.tile_size,
            tiles: source.tiles,
        };
        let bytes = serde_json::to_vec(&artifact).map_err(invalid)?;
        std::fs::write(output, bytes).map_err(invalid)
    }
}

fn invalid(error: impl std::fmt::Display) -> ImportError {
    ImportError::Other {
        reason: error.to_string(),
    }
}

pub(crate) fn validate_cells(cells: &[Vec<u32>]) -> Result<(), String> {
    let width = cells.first().map_or(0, Vec::len);
    if width == 0 || cells.iter().any(|row| row.len() != width) {
        return Err("Map cells must be a nonempty rectangular grid".into());
    }
    Ok(())
}

pub(crate) fn validate_tiles(tile_size: [f32; 2], tiles: &[TileData]) -> Result<(), String> {
    if tile_size
        .iter()
        .any(|size| !size.is_finite() || *size <= 0.0)
    {
        return Err("Tileset tile_size must contain positive finite sizes".into());
    }
    if tiles.is_empty() {
        return Err("Tileset must contain at least one tile".into());
    }
    let mut names = HashSet::new();
    for tile in tiles {
        if tile.name.trim().is_empty() || tile.name.chars().any(char::is_control) {
            return Err("Tile names must be nonempty and contain no control characters".into());
        }
        if !names.insert(&tile.name) {
            return Err(format!("Duplicate tile name: {}", tile.name));
        }
        let [x, y, width, height] = tile.region;
        if width == 0
            || height == 0
            || x.checked_add(width).is_none()
            || y.checked_add(height).is_none()
        {
            return Err(format!("Invalid region for tile: {}", tile.name));
        }
    }
    Ok(())
}
