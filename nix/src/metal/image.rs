pub struct Image {
    texture: metal::Texture,
}

impl Image {
    pub(super) fn new(texture: metal::Texture) -> Self {
        Image { texture }
    }
}
