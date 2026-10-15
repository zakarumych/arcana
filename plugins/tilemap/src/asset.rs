use std::sync::Arc;

use arcana::{
    assets::{Asset, AssetBuilder, AssetError, AssetVersion, Assets},
    graphics::texture::Texture,
};
use futures::future::poll_fn;

use crate::{
    MapData, TilesetData,
    importer::{validate_cells, validate_tiles},
};

/// Immutable map with its built tileset dependency.
#[derive(Clone, arcana::HasStid, arcana::ecs::component::Component)]
#[stid = "tilemap::Map"]
pub struct Map(Arc<MapInner>);

struct MapInner {
    data: MapData,
    tileset: Tileset,
}

impl Map {
    pub fn data(&self) -> &MapData {
        &self.0.data
    }

    pub fn tileset(&self) -> &Tileset {
        &self.0.tileset
    }
}

impl AssetVersion for Map {
    fn version() -> u64 {
        1
    }
}

impl Asset for Map {
    type Loaded = Self;

    async fn load(bytes: Box<[u8]>, assets: &Assets) -> Result<Self, AssetError> {
        let data: MapData = serde_json::from_slice(&bytes).map_err(AssetError::new)?;
        validate_cells(&data.cells).map_err(invalid)?;
        let tileset = poll_fn(|cx| assets.poll::<Tileset>(data.tileset, cx))
            .await
            .map_err(AssetError::new)?;
        if data
            .cells
            .iter()
            .flatten()
            .any(|&cell| u64::from(cell) > tileset.data().tiles.len() as u64)
        {
            return Err(invalid("Map cell index exceeds tileset tile count"));
        }
        Ok(Self(Arc::new(MapInner { data, tileset })))
    }

    fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
        Ok(loaded)
    }
}

/// Immutable tileset with its built texture dependency.
#[derive(Clone, arcana::HasStid)]
#[stid = "tilemap::Tileset"]
pub struct Tileset(Arc<TilesetInner>);

struct TilesetInner {
    data: TilesetData,
    texture: Texture,
}

impl Tileset {
    pub fn data(&self) -> &TilesetData {
        &self.0.data
    }

    pub fn texture(&self) -> &Texture {
        &self.0.texture
    }
}

impl AssetVersion for Tileset {
    fn version() -> u64 {
        1
    }
}

impl Asset for Tileset {
    type Loaded = Self;

    async fn load(bytes: Box<[u8]>, assets: &Assets) -> Result<Self, AssetError> {
        let data: TilesetData = serde_json::from_slice(&bytes).map_err(AssetError::new)?;
        validate_tiles(data.tile_size, &data.tiles).map_err(invalid)?;
        let texture = poll_fn(|cx| assets.poll::<Texture>(data.image, cx))
            .await
            .map_err(AssetError::new)?;
        let extent = texture.image.extent();
        for tile in &data.tiles {
            let [x, y, width, height] = tile.region;
            if x + width > extent.width() || y + height > extent.height() {
                return Err(invalid(format!(
                    "Tile region exceeds texture bounds: {}",
                    tile.name
                )));
            }
        }
        Ok(Self(Arc::new(TilesetInner { data, texture })))
    }

    fn build(loaded: Self, _: &mut AssetBuilder) -> Result<Self, AssetError> {
        Ok(loaded)
    }
}

fn invalid(error: impl std::fmt::Display) -> AssetError {
    AssetError::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        error.to_string(),
    ))
}
