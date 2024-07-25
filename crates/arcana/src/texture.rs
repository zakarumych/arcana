use edict::component::Component;

#[derive(Clone)]
pub struct Texture {
    pub image: mev::Image,
}

impl Component for Texture {
    fn name() -> &'static str {
        "Texture"
    }
}
