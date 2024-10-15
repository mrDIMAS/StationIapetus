use crate::{control_scheme::ControlScheme, MessageDirection, UiNode};
use fyrox::graph::SceneGraph;
use fyrox::{
    core::{algebra::Vector2, color::Color, log::Log, pool::Handle},
    gui::{brush::Brush, text::TextMessage, widget::WidgetMessage, UserInterface},
    renderer::{framework::gpu_texture::PixelKind, Renderer},
    resource::texture::{TextureResource, TextureResourceExtension},
};

#[derive(Default, Debug, Clone)]
pub struct DoorUi {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    logo: Handle<UiNode>,
    sector: Handle<UiNode>,
    text: Handle<UiNode>,
    action_text: Handle<UiNode>,
    need_render: bool,
}

impl DoorUi {
    pub fn new(mut ui: UserInterface) -> Self {
        ui.set_screen_size(Vector2::new(160.0, 160.0));
        let render_target = TextureResource::new_render_target(160, 160);
        Self {
            render_target,
            text: ui.find_handle_by_name_from_root("Text"),
            action_text: ui.find_handle_by_name_from_root("ActionText"),
            logo: ui.find_handle_by_name_from_root("Logo"),
            sector: ui.find_handle_by_name_from_root("Sector"),
            ui,
            need_render: true,
        }
    }

    pub fn update_text(
        &mut self,
        text: String,
        control_scheme: &ControlScheme,
        can_interact: bool,
        locked: bool,
    ) {
        self.ui.send_message(TextMessage::text(
            self.text,
            MessageDirection::ToWidget,
            text,
        ));

        if can_interact {
            self.ui.send_message(TextMessage::text(
                self.action_text,
                MessageDirection::ToWidget,
                format!("[{}] - Interact", control_scheme.action.button.name()),
            ));
        }

        self.ui.send_message(WidgetMessage::visibility(
            self.action_text,
            MessageDirection::ToWidget,
            can_interact,
        ));

        let brush = Brush::Solid(if locked { Color::RED } else { Color::GREEN });

        for widget in [self.action_text, self.text, self.sector] {
            self.ui.send_message(WidgetMessage::foreground(
                widget,
                MessageDirection::ToWidget,
                brush.clone(),
            ));
        }

        self.ui.send_message(WidgetMessage::background(
            self.logo,
            MessageDirection::ToWidget,
            brush.clone(),
        ));
    }

    pub fn set_color(&mut self, color: Color) {
        self.ui.send_message(WidgetMessage::foreground(
            self.text,
            MessageDirection::ToWidget,
            Brush::Solid(color),
        ));
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        if self.need_render {
            Log::verify(renderer.render_ui_to_texture(
                self.render_target.clone(),
                self.ui.screen_size(),
                self.ui.draw(),
                Color::TRANSPARENT,
                PixelKind::SRGBA8,
            ));
            self.need_render = false;
        }
    }

    pub fn update(&mut self, delta: f32) {
        let screen_size = self.ui.screen_size();
        self.ui.update(screen_size, delta, &Default::default());

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}
