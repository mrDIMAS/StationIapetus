use crate::elevator::{CallButton, CallButtonKind, Elevator};
use crate::{door::Door, MessageDirection, UiNode, WidgetBuilder};
use rg3d::gui::border::BorderBuilder;
use rg3d::{
    core::{algebra::Vector2, color::Color, pool::Handle},
    engine::resource_manager::ResourceManager,
    gui::{
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        text::TextBuilder,
        text::TextMessage,
        ttf::SharedFont,
        widget::WidgetMessage,
        HorizontalAlignment, Thickness, UserInterface, VerticalAlignment,
    },
    renderer::Renderer,
    resource::texture::Texture,
    utils::{into_gui_texture, log::Log},
};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

pub struct CallButtonUi {
    pub ui: UserInterface,
    pub render_target: Texture,
    floor_text: Handle<UiNode>,
    text: Handle<UiNode>,
}

impl CallButtonUi {
    pub const WIDTH: f32 = 100.0;
    pub const HEIGHT: f32 = 100.0;

    pub fn new(font: SharedFont, floor: u32) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));
        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let ctx = &mut ui.build_ctx();

        let text;
        let floor_text;

        BorderBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child({
                                floor_text = TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(0)
                                        .on_column(0)
                                        .with_margin(Thickness::top(25.0)),
                                )
                                .with_font(font.clone())
                                .with_horizontal_text_alignment(HorizontalAlignment::Center)
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .with_text(format!("Floor {}", floor))
                                .build(ctx);
                                floor_text
                            })
                            .with_child({
                                text = TextBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(1)
                                        .on_column(0)
                                        .with_foreground(Brush::Solid(Color::GREEN)),
                                )
                                .with_text("Call?")
                                .with_horizontal_text_alignment(HorizontalAlignment::Center)
                                .with_font(font)
                                .build(ctx);
                                text
                            }),
                    )
                    .add_column(Column::stretch())
                    .add_row(Row::stretch())
                    .add_row(Row::stretch())
                    .build(ctx),
                ),
        )
        .build(ctx);

        Self {
            ui,
            render_target,
            text,
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
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}

#[derive(Default)]
pub struct CallButtonUiContainer {
    map: HashMap<u64, CallButtonUi>,
}

fn calc_hash(elevator_handle: Handle<Elevator>, call_button_handle: Handle<CallButton>) -> u64 {
    let mut hasher = DefaultHasher::new();
    elevator_handle.hash(&mut hasher);
    call_button_handle.hash(&mut hasher);
    hasher.finish()
}

impl CallButtonUiContainer {
    pub fn create_ui(
        &mut self,
        font: SharedFont,
        elevator_handle: Handle<Elevator>,
        call_button_handle: Handle<CallButton>,
        call_button: &CallButton,
    ) -> Texture {
        let ui = CallButtonUi::new(font, call_button.floor);
        let texture = ui.render_target.clone();
        assert!(self
            .map
            .insert(calc_hash(elevator_handle, call_button_handle), ui)
            .is_none());
        texture
    }

    pub fn get_ui_mut(
        &mut self,
        elevator_handle: Handle<Elevator>,
        call_button_handle: Handle<CallButton>,
    ) -> Option<&mut CallButtonUi> {
        self.map
            .get_mut(&calc_hash(elevator_handle, call_button_handle))
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
