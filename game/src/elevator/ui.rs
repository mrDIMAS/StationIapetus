use crate::{
    gui,
    ui_container::{InteractiveUi, UiContainer},
    MessageDirection, UiNode, WidgetBuilder,
};
use fyrox::gui::font::FontResource;
use fyrox::resource::texture::TextureResource;
use fyrox::{
    core::{algebra::Vector2, color::Color, pool::Handle},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        text::{TextBuilder, TextMessage},
        HorizontalAlignment, Thickness, UserInterface, VerticalAlignment,
    },
    scene::node::Node,
};

pub struct CallButtonUi {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    floor_text: Handle<UiNode>,
    text: Handle<UiNode>,
}

impl InteractiveUi for CallButtonUi {
    fn ui(&mut self) -> &mut UserInterface {
        &mut self.ui
    }

    fn texture(&self) -> TextureResource {
        self.render_target.clone()
    }

    fn update(&mut self, delta: f32) {
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}

impl CallButtonUi {
    pub const WIDTH: f32 = 100.0;
    pub const HEIGHT: f32 = 100.0;

    pub fn new(font: FontResource, floor: u32) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));
        let render_target = gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT);

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
                                .with_height(31.0)
                                .with_font(font.clone())
                                .with_horizontal_text_alignment(HorizontalAlignment::Center)
                                .with_vertical_text_alignment(VerticalAlignment::Center)
                                .with_text(format!("Floor {floor}"))
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
                                .with_height(31.0)
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
}

pub type CallButtonUiContainer = UiContainer<Node, CallButtonUi>;

impl CallButtonUiContainer {
    pub fn create_ui(
        &mut self,
        font: FontResource,
        call_button_handle: Handle<Node>,
        floor: u32,
    ) -> TextureResource {
        self.add(call_button_handle, CallButtonUi::new(font, floor))
    }
}
