use crate::{control_scheme::ControlScheme, gui, level::item::Item};
use fyrox::{
    core::{algebra::Vector2, color::Color, pool::Handle, visitor::prelude::*},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        font::FontResource,
        grid::{Column, GridBuilder, Row},
        image::{ImageBuilder, ImageMessage},
        message::MessageDirection,
        text::{TextBuilder, TextMessage},
        widget::WidgetBuilder,
        HorizontalAlignment, UiNode, UserInterface, VerticalAlignment,
    },
    resource::{model::ModelResource, texture::TextureResource},
};
use std::ops::Deref;

#[derive(Visit, Default, Debug)]
pub struct ItemDisplay {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    item_image: Handle<UiNode>,
    item_name: Handle<UiNode>,
    action_text: Handle<UiNode>,
    pub current_item: Option<ModelResource>,
}

impl ItemDisplay {
    pub const WIDTH: f32 = 250.0;
    pub const HEIGHT: f32 = 300.0;

    pub fn new(font: FontResource) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT);

        let item_image;
        let item_name;
        let action_text;
        GridBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    BorderBuilder::new(
                        WidgetBuilder::new()
                            .with_foreground(Brush::Solid(Color::WHITE).into())
                            .with_background(Brush::Solid(Color::opaque(120, 120, 120)).into())
                            .with_child({
                                item_image = ImageBuilder::new(
                                    WidgetBuilder::new()
                                        .with_background(Brush::Solid(Color::WHITE).into())
                                        .with_foreground(Brush::Solid(Color::WHITE).into())
                                        .with_width(170.0)
                                        .with_height(170.0)
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
                    .with_font(font.clone())
                    .with_font_size(30.0.into())
                    .build(&mut ui.build_ctx());
                    item_name
                })
                .with_child({
                    action_text = TextBuilder::new(
                        WidgetBuilder::new()
                            .with_foreground(Brush::Solid(Color::GREEN).into())
                            .with_vertical_alignment(VerticalAlignment::Center)
                            .with_horizontal_alignment(HorizontalAlignment::Center)
                            .on_row(2)
                            .on_column(0),
                    )
                    .with_shadow(true)
                    .with_font_size(20.0.into())
                    .with_font(font)
                    .build(&mut ui.build_ctx());
                    action_text
                }),
        )
        .add_column(Column::stretch())
        .add_row(Row::stretch())
        .add_row(Row::auto())
        .add_row(Row::auto())
        .build(&mut ui.build_ctx());

        Self {
            ui,
            render_target,
            item_image,
            item_name,
            action_text,
            current_item: None,
        }
    }

    pub fn sync_to_model(
        &mut self,
        item: ModelResource,
        count: u32,
        control_scheme: &ControlScheme,
    ) {
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
                        item_script.preview.deref().clone().map(Into::into),
                    ));
                }
            });

            self.ui.send_message(TextMessage::text(
                self.action_text,
                MessageDirection::ToWidget,
                format!("[{}] - Pickup", control_scheme.action.button.name()),
            ));
        }
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
}
