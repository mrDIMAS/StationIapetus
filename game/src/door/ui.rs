use crate::{
    ui_container::{InteractiveUi, UiContainer},
    MessageDirection, UiNode, WidgetBuilder,
};
use fyrox::resource::texture::{TextureResource, TextureResourceExtension};
use fyrox::scene::node::Node;
use fyrox::{
    asset::manager::ResourceManager,
    core::{algebra::Vector2, color::Color, pool::Handle},
    gui::{
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        text::{TextBuilder, TextMessage},
        ttf::SharedFont,
        widget::WidgetMessage,
        HorizontalAlignment, Thickness, UserInterface, VerticalAlignment,
    },
    resource::texture::Texture,
    utils::into_gui_texture,
};

pub struct DoorUi {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    text: Handle<UiNode>,
}

impl InteractiveUi for DoorUi {
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

impl DoorUi {
    pub const WIDTH: f32 = 160.0;
    pub const HEIGHT: f32 = 160.0;

    pub fn new(font: SharedFont, resource_manager: ResourceManager) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));
        let render_target =
            TextureResource::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

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
                        resource_manager.request::<Texture, _>("data/ui/triangles.png"),
                    ))
                    .build(ctx),
                )
                .with_child(
                    TextBuilder::new(
                        WidgetBuilder::new()
                            .on_row(0)
                            .on_column(0)
                            .with_margin(Thickness::top(25.0)),
                    )
                    .with_font(font.clone())
                    .with_horizontal_text_alignment(HorizontalAlignment::Center)
                    .with_vertical_text_alignment(VerticalAlignment::Center)
                    .with_text("D")
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
}

pub type DoorUiContainer = UiContainer<Node, DoorUi>;

impl DoorUiContainer {
    pub fn create_ui(
        &mut self,
        font: SharedFont,
        resource_manager: ResourceManager,
        door_handle: Handle<Node>,
    ) -> TextureResource {
        self.add(door_handle, DoorUi::new(font, resource_manager))
    }
}
