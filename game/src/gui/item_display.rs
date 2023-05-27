use crate::level::item::Item;
use fyrox::{
    core::{algebra::Vector2, color::Color, pool::Handle},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        image::{ImageBuilder, ImageMessage},
        message::MessageDirection,
        text::{TextBuilder, TextMessage},
        ttf::SharedFont,
        widget::WidgetBuilder,
        HorizontalAlignment, UiNode, UserInterface, VerticalAlignment,
    },
    resource::{
        model::ModelResource,
        texture::{TextureResource, TextureResourceExtension},
    },
};

pub struct ItemDisplay {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    item_image: Handle<UiNode>,
    item_name: Handle<UiNode>,
    pub current_item: Option<ModelResource>,
}

impl ItemDisplay {
    pub const WIDTH: f32 = 128.0;
    pub const HEIGHT: f32 = 160.0;

    pub fn new(font: SharedFont) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target =
            TextureResource::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let item_image;
        let item_name;
        GridBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    BorderBuilder::new(
                        WidgetBuilder::new()
                            .with_foreground(Brush::Solid(Color::WHITE))
                            .with_background(Brush::Solid(Color::opaque(120, 120, 120)))
                            .with_child({
                                item_image = ImageBuilder::new(
                                    WidgetBuilder::new()
                                        .with_background(Brush::Solid(Color::WHITE))
                                        .with_foreground(Brush::Solid(Color::WHITE))
                                        .with_width(128.0)
                                        .with_height(128.0)
                                        .on_row(0)
                                        .on_column(0),
                                )
                                .build(&mut ui.build_ctx());
                                item_image
                            }),
                    )
                    .build(&mut ui.build_ctx()),
                )
                .with_child({
                    item_name = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .on_row(1)
                            .on_column(0),
                    )
                    .with_font(font)
                    .build(&mut ui.build_ctx());
                    item_name
                }),
        )
        .add_column(Column::auto())
        .add_row(Row::stretch())
        .add_row(Row::auto())
        .build(&mut ui.build_ctx());

        Self {
            ui,
            render_target,
            item_image,
            item_name,
            current_item: None,
        }
    }

    pub fn sync_to_model(&mut self, item: ModelResource, count: u32) {
        if self
            .current_item
            .as_ref()
            .map_or(true, |current_item| current_item != &item)
        {
            self.current_item = Some(item.clone());

            Item::from_resource(&item, |item| {
                if let Some(item_script) = item {
                    self.ui.send_message(TextMessage::text(
                        self.item_name,
                        MessageDirection::ToWidget,
                        format!("{}-{}", *item_script.name, count),
                    ));

                    self.ui.send_message(ImageMessage::texture(
                        self.item_image,
                        MessageDirection::ToWidget,
                        item_script
                            .preview
                            .as_ref()
                            .map(|tex| fyrox::utils::into_gui_texture(tex.clone())),
                    ));
                }
            });
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}
