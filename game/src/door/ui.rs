use crate::{control_scheme::ControlScheme, UiNode};
use fyrox::asset::manager::ResourceManager;
use fyrox::gui::texture::TexturePixelKind;
use fyrox::renderer::ui_renderer::UiRenderInfo;
use fyrox::{
    core::{algebra::Vector2, color::Color, log::Log, pool::Handle},
    graph::SceneGraph,
    gui::{
        brush::Brush,
        message::UiMessage,
        text::{Text, TextMessage},
        widget::{Widget, WidgetMessage},
        UserInterface,
    },
    renderer::Renderer,
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
        let render_target =
            TextureResource::new_render_target_with_format(160, 160, TexturePixelKind::BGR8);
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
        self.try_update_text(self.text, text);

        if can_interact {
            self.try_update_text(
                self.action_text,
                format!("[{}] - Interact", control_scheme.action.button.name()),
            );
        }

        self.try_update_visibility(self.action_text, can_interact);

        let brush = Brush::Solid(if locked { Color::RED } else { Color::GREEN });
        for widget in [self.action_text, self.text, self.sector] {
            self.try_update_foreground(widget, brush.clone());
        }

        self.try_update_background(self.logo, brush.clone());
    }

    fn try_update_widget_value<Widget: 'static, Param: PartialEq>(
        &mut self,
        widget: Handle<UiNode>,
        value: Param,
        get: impl Fn(&Widget) -> Param,
        make_msg: impl FnOnce(Handle<UiNode>, Param) -> UiMessage,
    ) {
        if self
            .ui
            .try_get_of_type::<Widget>(widget)
            .is_some_and(|widget_ref| get(widget_ref) != value)
        {
            self.ui.send_message(make_msg(widget, value));
            self.need_render = true;
        }
    }

    fn try_update_text(&mut self, widget: Handle<UiNode>, text: String) {
        self.try_update_widget_value::<Text, _>(
            widget,
            text,
            |w| w.text(),
            |h, value| UiMessage::for_widget(h, TextMessage::Text(value)),
        )
    }

    fn try_update_background(&mut self, widget: Handle<UiNode>, brush: Brush) {
        self.try_update_widget_value::<Widget, _>(
            widget,
            brush,
            |w| w.background(),
            |h, value| UiMessage::for_widget(h, WidgetMessage::Background(value.into())),
        )
    }

    fn try_update_foreground(&mut self, widget: Handle<UiNode>, brush: Brush) {
        self.try_update_widget_value::<Widget, _>(
            widget,
            brush,
            |w| w.foreground(),
            |h, value| UiMessage::for_widget(h, WidgetMessage::Foreground(value.into())),
        )
    }

    fn try_update_visibility(&mut self, widget: Handle<UiNode>, visibility: bool) {
        self.try_update_widget_value::<Widget, _>(
            widget,
            visibility,
            |w| w.visibility(),
            |h, value| UiMessage::for_widget(h, WidgetMessage::Visibility(value)),
        )
    }

    pub fn render(&mut self, renderer: &mut Renderer, resource_manager: &ResourceManager) {
        if self.need_render {
            self.ui.draw();
            Log::verify(renderer.render_ui(UiRenderInfo {
                ui: &self.ui,
                render_target: Some(self.render_target.clone()),
                clear_color: Color::TRANSPARENT,
                resource_manager,
            }));
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
