use crate::{door::Door, MessageDirection, UiNode, WidgetBuilder};
use rg3d::core::color::Color;
use rg3d::gui::brush::Brush;
use rg3d::gui::widget::WidgetMessage;
use rg3d::{
    core::{algebra::Vector2, pool::Handle},
    engine::resource_manager::ResourceManager,
    gui::{
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        text::TextBuilder,
        text::TextMessage,
        ttf::SharedFont,
        HorizontalAlignment, UserInterface,
    },
    renderer::Renderer,
    resource::texture::Texture,
    utils::{into_gui_texture, log::Log},
};
use std::collections::HashMap;

pub struct DoorUi {
    pub ui: UserInterface,
    pub render_target: Texture,
    text: Handle<UiNode>,
}

impl DoorUi {
    pub const WIDTH: f32 = 160.0;
    pub const HEIGHT: f32 = 160.0;

    pub fn new(font: SharedFont, resource_manager: ResourceManager) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));
        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let ctx = &mut ui.build_ctx();

        let text;
        GridBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    ImageBuilder::new(
                        WidgetBuilder::new()
                            .with_width(120.0)
                            .with_height(120.0)
                            .on_row(0)
                            .on_column(0),
                    )
                    .with_texture(into_gui_texture(
                        resource_manager.request_texture("data/ui/triangles.png", None),
                    ))
                    .build(ctx),
                )
                .with_child({
                    text = TextBuilder::new(
                        WidgetBuilder::new()
                            .on_row(1)
                            .on_column(0)
                            .with_foreground(Brush::Solid(Color::GREEN)),
                    )
                    .with_horizontal_text_alignment(HorizontalAlignment::Center)
                    .with_font(font)
                    .build(ctx);
                    text
                }),
        )
        .add_column(Column::stretch())
        .add_row(Row::stretch())
        .add_row(Row::auto())
        .build(ctx);

        Self {
            ui,
            render_target,
            text,
        }
    }

    pub fn set_text(&mut self, text: String) {
        self.ui.send_message(TextMessage::text(
            self.text,
            MessageDirection::ToWidget,
            text,
        ));
    }

    pub fn set_color(&mut self, color: Color) {
        self.ui.send_message(WidgetMessage::foreground(
            self.text,
            MessageDirection::ToWidget,
            Brush::Solid(color),
        ));
    }

    pub fn update(&mut self, delta: f32) {
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}

#[derive(Default)]
pub struct DoorUiContainer {
    map: HashMap<Handle<Door>, DoorUi>,
}

impl DoorUiContainer {
    pub fn create_ui(
        &mut self,
        font: SharedFont,
        resource_manager: ResourceManager,
        door_handle: Handle<Door>,
    ) -> Texture {
        let ui = DoorUi::new(font, resource_manager);
        let texture = ui.render_target.clone();
        assert!(self.map.insert(door_handle, ui).is_none());
        texture
    }

    pub fn get_ui_mut(&mut self, door_handle: Handle<Door>) -> Option<&mut DoorUi> {
        self.map.get_mut(&door_handle)
    }

    pub fn render(&mut self, renderer: &mut Renderer) {
        for ui in self.map.values_mut() {
            Log::verify(renderer.render_ui_to_texture(ui.render_target.clone(), &mut ui.ui));
        }
    }

    pub fn update(&mut self, delta: f32) {
        for ui in self.map.values_mut() {
            ui.update(delta);
        }
    }

    pub fn clear(&mut self) {
        self.map.clear()
    }
}
