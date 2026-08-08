use crate::{gui, UiNode};
use fyrox::asset::manager::ResourceManager;
use fyrox::graph::SceneGraph;
use fyrox::renderer::ui_renderer::UiRenderInfo;
use fyrox::{
    core::{algebra::Vector2, color::Color, log::Log, pool::Handle},
    gui::{text::TextMessage, UserInterface},
    renderer::Renderer,
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
        ui.send(floor_text, TextMessage::Text(format!("Floor {floor}")));
        Self {
            text: ui.find_handle_by_name_from_root("Text"),
            ui,
            render_target: gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT),
            floor_text,
        }
    }

    pub fn set_text(&mut self, text: String) {
        self.ui.send(self.text, TextMessage::Text(text));
    }

    pub fn set_floor_text(&mut self, text: String) {
        self.ui.send(self.floor_text, TextMessage::Text(text));
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

    pub fn render(&mut self, renderer: &mut Renderer, resource_manager: &ResourceManager) {
        Log::verify(renderer.render_ui(UiRenderInfo {
            ui: &self.ui,
            render_target: Some(self.render_target.clone()),
            clear_color: Color::TRANSPARENT,
            resource_manager,
        }));
    }
}
