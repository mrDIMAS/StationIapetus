use crate::{gui, MessageDirection, UiNode};
use fyrox::graph::SceneGraph;
use fyrox::{
    core::{algebra::Vector2, color::Color, log::Log, pool::Handle},
    gui::{text::TextMessage, UserInterface},
    renderer::{framework::gpu_texture::PixelKind, Renderer},
    resource::texture::TextureResource,
};

#[derive(Debug, Clone)]
pub struct CallButtonUi {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    floor_text: Handle<UiNode>,
    text: Handle<UiNode>,
}

impl CallButtonUi {
    pub const WIDTH: f32 = 100.0;
    pub const HEIGHT: f32 = 100.0;

    pub fn new(ui: UserInterface, floor: u32) -> Self {
        let floor_text = ui.find_handle_by_name_from_root("FloorText");
        ui.send_message(TextMessage::text(
            floor_text,
            MessageDirection::ToWidget,
            format!("Floor {floor}"),
        ));
        Self {
            text: ui.find_handle_by_name_from_root("Text"),
            ui,
            render_target: gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT),
            floor_text,
        }
    }

    pub fn set_text(&mut self, text: String) {
        self.ui.send_message(TextMessage::text(
            self.text,
            MessageDirection::ToWidget,
            text,
        ));
    }

    pub fn set_floor_text(&mut self, text: String) {
        self.ui.send_message(TextMessage::text(
            self.floor_text,
            MessageDirection::ToWidget,
            text,
        ));
    }

    pub fn update(&mut self, delta: f32) {
        self.ui.update(
            Vector2::new(Self::WIDTH, Self::HEIGHT),
            delta,
            &Default::default(),
        );

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        Log::verify(renderer.render_ui_to_texture(
            self.render_target.clone(),
            self.ui.screen_size(),
            self.ui.draw(),
            Color::TRANSPARENT,
            PixelKind::SRGBA8,
        ));
    }
}
