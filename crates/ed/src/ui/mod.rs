use arboard::Clipboard;
use arcana::{gametime::TimeStamp, mev};
use hashbrown::HashMap;
use render::Render;
use winit::window::Window;

mod events;
mod render;

#[derive(Clone, Copy)]
pub enum Sampler {
    NearestNearest = 0,
    NearestLinear = 1,
    LinearNearest = 2,
    LinearLinear = 3,
}

impl Sampler {
    fn from_options(options: egui::TextureOptions) -> Self {
        match (options.minification, options.magnification) {
            (egui::TextureFilter::Nearest, egui::TextureFilter::Nearest) => Sampler::NearestNearest,
            (egui::TextureFilter::Nearest, egui::TextureFilter::Linear) => Sampler::NearestLinear,
            (egui::TextureFilter::Linear, egui::TextureFilter::Nearest) => Sampler::LinearNearest,
            (egui::TextureFilter::Linear, egui::TextureFilter::Linear) => Sampler::LinearLinear,
        }
    }
}

pub struct UiViewport {
    id: egui::ViewportId,
    raw_input: egui::RawInput,
    mouse_pos: egui::Pos2,
    scale_factor: f32,
    size: egui::Vec2,
    shapes: Vec<egui::epaint::ClippedShape>,
}

pub struct UserTextures<'a> {
    textures: &'a mut HashMap<egui::TextureId, (mev::Image, Sampler)>,
    next_user_texture_id: &'a mut u64,
}

impl UserTextures<'_> {
    pub fn new_id(&mut self) -> egui::TextureId {
        let id = *self.next_user_texture_id;
        *self.next_user_texture_id += 1;
        egui::TextureId::User(id)
    }

    pub fn add(&mut self, image: mev::Image, sampler: Sampler) -> egui::TextureId {
        let id = *self.next_user_texture_id;
        *self.next_user_texture_id += 1;
        self.textures
            .insert(egui::TextureId::User(id), (image, sampler));
        egui::TextureId::User(id)
    }

    pub fn set(&mut self, id: egui::TextureId, image: mev::Image, sampler: Sampler) {
        assert!(matches!(id, egui::TextureId::User(_)));

        self.textures.insert(id, (image, sampler));
    }
}

pub struct Ui {
    cx: egui::Context,
    next_id: egui::Id,
    textures: HashMap<egui::TextureId, (mev::Image, Sampler)>,
    textures_delta: egui::TexturesDelta,
    cursor: egui::CursorIcon,
    render: Render,
    next_user_texture_id: u64,
}

impl Ui {
    pub fn new() -> Self {
        let cx = egui::Context::default();
        cx.set_style(arcana_style());
        cx.set_embed_viewports(false);

        let mut fonts = egui::FontDefinitions::default();
        egui_phosphor::add_to_fonts(&mut fonts, egui_phosphor::Variant::Regular);
        cx.set_fonts(fonts);

        Ui {
            cx,
            textures_delta: egui::TexturesDelta::default(),
            textures: HashMap::new(),
            next_id: egui::Id::new("arcana-0"),
            cursor: egui::CursorIcon::Default,
            render: Render::new(),
            next_user_texture_id: 0,
        }
    }

    pub fn new_viewport(&mut self, size: egui::Vec2, scale_factor: f32) -> UiViewport {
        let id: egui::ViewportId = egui::ViewportId(self.next_id);
        self.next_id = self.next_id.with("next_id");

        self._new_viewport(id, size, scale_factor)
    }

    pub fn main_viewport(&mut self, size: egui::Vec2, scale_factor: f32) -> UiViewport {
        let id: egui::ViewportId = egui::ViewportId::ROOT;
        self._new_viewport(id, size, scale_factor)
    }

    fn _new_viewport(
        &mut self,
        id: egui::ViewportId,
        size: egui::Vec2,
        scale_factor: f32,
    ) -> UiViewport {
        let mut raw_input = egui::RawInput::default();
        raw_input.viewport_id = id;
        raw_input.screen_rect = Some(egui::Rect::from_min_size(
            Default::default(),
            size / scale_factor,
        ));

        let vp_info = raw_input.viewports.entry(id).or_default();
        vp_info.native_pixels_per_point = Some(scale_factor);

        vp_info.inner_rect = raw_input.screen_rect;

        self.cx.request_repaint_of(id);
        UiViewport {
            id,
            raw_input,
            mouse_pos: egui::Pos2::ZERO,
            scale_factor,
            size,
            shapes: Vec::new(),
        }
    }

    pub fn textures(&mut self) -> UserTextures {
        UserTextures {
            textures: &mut self.textures,
            next_user_texture_id: &mut self.next_user_texture_id,
        }
    }

    pub fn run(
        &mut self,
        viewport: &mut UiViewport,
        clipboard: &mut Clipboard,
        window: &Window,
        time: TimeStamp,
        run_ui: impl FnOnce(&egui::Context, UserTextures),
    ) {
        viewport.raw_input.time = Some(time.elapsed_since_start().as_secs_f64());

        let user_textures = UserTextures {
            textures: &mut self.textures,
            next_user_texture_id: &mut self.next_user_texture_id,
        };

        let output = self.cx.run(viewport.raw_input.take(), |cx| {
            run_ui(cx, user_textures);
        });

        assert_eq!(output.pixels_per_point, viewport.scale_factor);

        handle_platform_output(output.platform_output, window, &mut self.cursor, clipboard);

        self.textures_delta.append(output.textures_delta);
        viewport.shapes = output.shapes;
    }

    pub fn render(&mut self, viewport: &mut UiViewport, frame: mev::Frame, queue: &mut mev::Queue) {
        let r = self.render.render(
            &self.cx,
            frame,
            queue,
            &mut self.textures,
            &mut self.textures_delta,
            std::mem::take(&mut viewport.shapes),
            viewport.scale_factor,
        );

        if let Err(err) = r {
            tracing::error!("UI render error: {}", err);
        }
    }
}

fn arcana_style() -> egui::Style {
    let mut style = egui::Style::default();

    style.visuals = egui::Visuals::dark();

    // style.visuals.widgets.noninteractive.bg_fill = Color32::WHITE;
    // style.visuals.widgets.noninteractive.weak_bg_fill = Color32::WHITE;
    // style.visuals.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, Color32::TRANSPARENT);
    // style.visuals.widgets.noninteractive.rounding = egui::Rounding::ZERO;
    // style.visuals.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, Color32::BLACK);
    // style.visuals.widgets.noninteractive.expansion = 0.0;

    // style.visuals.widgets.inactive.bg_fill = Color32::GRAY;
    // style.visuals.widgets.inactive.weak_bg_fill = Color32::GRAY;
    // style.visuals.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, Color32::TRANSPARENT);
    // style.visuals.widgets.inactive.rounding = egui::Rounding::ZERO;
    // style.visuals.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, Color32::BLACK);
    // style.visuals.widgets.inactive.expansion = 0.0;

    // style.visuals.widgets.hovered.bg_fill = Color32::YELLOW;
    // style.visuals.widgets.hovered.weak_bg_fill = Color32::LIGHT_YELLOW;
    // style.visuals.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, Color32::TRANSPARENT);
    // style.visuals.widgets.hovered.rounding = egui::Rounding::ZERO;
    // style.visuals.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, Color32::BLACK);
    // style.visuals.widgets.hovered.expansion = 0.0;

    // style.visuals.widgets.active.bg_fill = Color32::WHITE;
    // style.visuals.widgets.active.weak_bg_fill = Color32::WHITE;
    // style.visuals.widgets.active.bg_stroke = egui::Stroke::new(1.0, Color32::TRANSPARENT);
    // style.visuals.widgets.active.rounding = egui::Rounding::ZERO;
    // style.visuals.widgets.active.fg_stroke = egui::Stroke::new(1.0, Color32::BLACK);
    // style.visuals.widgets.active.expansion = 0.0;

    // style.visuals.widgets.open.bg_fill = Color32::WHITE;
    // style.visuals.widgets.open.weak_bg_fill = Color32::WHITE;
    // style.visuals.widgets.open.bg_stroke = egui::Stroke::new(1.0, Color32::TRANSPARENT);
    // style.visuals.widgets.open.rounding = egui::Rounding::ZERO;
    // style.visuals.widgets.open.fg_stroke = egui::Stroke::new(1.0, Color32::BLACK);
    // style.visuals.widgets.open.expansion = 0.0;

    // style.visuals.selection.bg_fill = Color32::LIGHT_YELLOW;
    // style.visuals.selection.stroke = egui::Stroke::new(1.0, Color32::BLACK);

    // style.visuals.faint_bg_color = Color32::LIGHT_GRAY;
    // style.visuals.extreme_bg_color = Color32::WHITE;
    // style.visuals.code_bg_color = Color32::WHITE;
    // style.visuals.warn_fg_color = Color32::RED;
    // style.visuals.error_fg_color = Color32::DARK_RED;

    // style.visuals.window_rounding = egui::Rounding::ZERO;
    // style.visuals.window_shadow = egui::epaint::Shadow::NONE;
    // style.visuals.window_fill = Color32::WHITE;
    // style.visuals.window_stroke = egui::Stroke::new(2.0, Color32::TRANSPARENT);

    // style.visuals.window_highlight_topmost = true;

    // style.visuals.menu_rounding = egui::Rounding::ZERO;

    // style.visuals.panel_fill = Color32::WHITE;

    // style.visuals.popup_shadow = egui::epaint::Shadow::NONE;

    // style.visuals.resize_corner_size = 12.0;

    // style.visuals.text_cursor = egui::Stroke::new(1.0, Color32::BLACK);
    // style.visuals.text_cursor_preview = true;

    // style.visuals.clip_rect_margin = 0.0;

    // style.visuals.button_frame = true;

    // style.visuals.collapsing_header_frame = true;
    // style.visuals.indent_has_left_vline = true;

    // style.visuals.striped = true;
    // style.visuals.slider_trailing_fill = false;

    // style.visuals.handle_shape = egui::style::HandleShape::Rect { aspect_ratio: 1.0 };

    // style.visuals.interact_cursor = None;

    // style.visuals.image_loading_spinners = true;

    style
}

fn map_cursor(icon: egui::CursorIcon) -> Option<cursor_icon::CursorIcon> {
    match icon {
        egui::CursorIcon::Default => Some(cursor_icon::CursorIcon::Default),
        egui::CursorIcon::None => None,
        egui::CursorIcon::ContextMenu => Some(cursor_icon::CursorIcon::ContextMenu),
        egui::CursorIcon::Help => Some(cursor_icon::CursorIcon::Help),
        egui::CursorIcon::PointingHand => Some(cursor_icon::CursorIcon::Pointer),
        egui::CursorIcon::Progress => Some(cursor_icon::CursorIcon::Progress),
        egui::CursorIcon::Wait => Some(cursor_icon::CursorIcon::Wait),
        egui::CursorIcon::Cell => Some(cursor_icon::CursorIcon::Cell),
        egui::CursorIcon::Crosshair => Some(cursor_icon::CursorIcon::Crosshair),
        egui::CursorIcon::Text => Some(cursor_icon::CursorIcon::Text),
        egui::CursorIcon::VerticalText => Some(cursor_icon::CursorIcon::VerticalText),
        egui::CursorIcon::Alias => Some(cursor_icon::CursorIcon::Alias),
        egui::CursorIcon::Copy => Some(cursor_icon::CursorIcon::Copy),
        egui::CursorIcon::Move => Some(cursor_icon::CursorIcon::Move),
        egui::CursorIcon::NoDrop => Some(cursor_icon::CursorIcon::NoDrop),
        egui::CursorIcon::NotAllowed => Some(cursor_icon::CursorIcon::NotAllowed),
        egui::CursorIcon::Grab => Some(cursor_icon::CursorIcon::Grab),
        egui::CursorIcon::Grabbing => Some(cursor_icon::CursorIcon::Grabbing),
        egui::CursorIcon::AllScroll => Some(cursor_icon::CursorIcon::AllScroll),
        egui::CursorIcon::ResizeHorizontal => Some(cursor_icon::CursorIcon::EwResize),
        egui::CursorIcon::ResizeNeSw => Some(cursor_icon::CursorIcon::NeswResize),
        egui::CursorIcon::ResizeNwSe => Some(cursor_icon::CursorIcon::NwseResize),
        egui::CursorIcon::ResizeVertical => Some(cursor_icon::CursorIcon::NsResize),
        egui::CursorIcon::ResizeEast => Some(cursor_icon::CursorIcon::EResize),
        egui::CursorIcon::ResizeSouthEast => Some(cursor_icon::CursorIcon::SeResize),
        egui::CursorIcon::ResizeSouth => Some(cursor_icon::CursorIcon::SResize),
        egui::CursorIcon::ResizeSouthWest => Some(cursor_icon::CursorIcon::SwResize),
        egui::CursorIcon::ResizeWest => Some(cursor_icon::CursorIcon::WResize),
        egui::CursorIcon::ResizeNorthWest => Some(cursor_icon::CursorIcon::NwResize),
        egui::CursorIcon::ResizeNorth => Some(cursor_icon::CursorIcon::NResize),
        egui::CursorIcon::ResizeNorthEast => Some(cursor_icon::CursorIcon::NeResize),
        egui::CursorIcon::ResizeColumn => Some(cursor_icon::CursorIcon::ColResize),
        egui::CursorIcon::ResizeRow => Some(cursor_icon::CursorIcon::RowResize),
        egui::CursorIcon::ZoomIn => Some(cursor_icon::CursorIcon::ZoomIn),
        egui::CursorIcon::ZoomOut => Some(cursor_icon::CursorIcon::ZoomOut),
    }
}

fn handle_platform_output(
    output: egui::PlatformOutput,
    window: &Window,
    cursor: &mut egui::CursorIcon,
    clipboard: &mut Clipboard,
) {
    if *cursor != output.cursor_icon {
        *cursor = output.cursor_icon;

        match map_cursor(output.cursor_icon) {
            None => window.set_cursor_visible(false),
            Some(cursor) => {
                window.set_cursor_visible(true);
                window.set_cursor(cursor);
            }
        }
    }

    if let Some(url) = output.open_url {
        if let Err(err) = open::that_detached(url.url) {
            tracing::error!("Failed to open URL: {}", err);
        }
    }

    if !output.copied_text.is_empty() {
        if let Err(err) = clipboard.set_text(output.copied_text) {
            tracing::error!("Failed to set clipboard text: {}", err);
        }
    }
}
