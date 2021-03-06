use crate::{
    gui::{Gui, UiNode},
    item::{Item, ItemKind},
};
use rg3d::{
    core::{algebra::Vector2, pool::Handle},
    engine::resource_manager::ResourceManager,
    gui::{
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        message::{MessageDirection, TextMessage},
        text::TextBuilder,
        ttf::SharedFont,
        widget::WidgetBuilder,
        HorizontalAlignment, VerticalAlignment,
    },
    resource::texture::Texture,
};

pub struct ItemDisplay {
    pub ui: Gui,
    pub render_target: Texture,
    item_image: Handle<UiNode>,
    item_name: Handle<UiNode>,
}

impl ItemDisplay {
    pub const WIDTH: f32 = 128.0;
    pub const HEIGHT: f32 = 160.0;

    pub fn new(font: SharedFont, resource_manager: ResourceManager) -> Self {
        let mut ui = Gui::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let item_image;
        let item_name;
        GridBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child({
                    item_image = ImageBuilder::new(
                        WidgetBuilder::new()
                            .with_width(128.0)
                            .with_height(128.0)
                            .on_row(0)
                            .on_column(0),
                    )
                    .build(&mut ui.build_ctx());
                    item_image
                })
                .with_child({
                    item_name = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .on_row(1)
                            .on_column(0),
                    )
                    .with_font(font.clone())
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
        }
    }

    pub fn sync_to_model(&self, item: ItemKind, count: u32) {
        let definition = Item::get_definition(item);

        self.ui.send_message(TextMessage::text(
            self.item_name,
            MessageDirection::ToWidget,
            format!("{}-{}", definition.name, count),
        ));

        // TODO: Sync image.
    }

    pub fn update(&mut self, delta: f32) {
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        // Just pump all messages, but ignore them in game code.
        while self.ui.poll_message().is_some() {}
    }
}
