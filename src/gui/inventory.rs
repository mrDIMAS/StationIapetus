use crate::{
    gui::{
        BuildContext, CustomUiMessage, CustomUiNode, CustomWidget, Gui, UiNode, UiWidgetBuilder,
    },
    item::{Item, ItemKind},
    player::Player,
};
use rg3d::{
    core::{algebra::Vector2, color::Color, pool::Handle},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        message::{MessageDirection, UiMessage, WidgetMessage},
        scroll_viewer::ScrollViewerBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        wrap_panel::WrapPanelBuilder,
        Control, HorizontalAlignment, Orientation, Thickness, UserInterface, VerticalAlignment,
    },
    resource::texture::Texture,
};
use std::ops::{Deref, DerefMut};

pub struct InventoryInterface {
    pub ui: Gui,
    pub render_target: Texture,
    items_panel: Handle<UiNode>,
}

#[derive(Debug, Clone)]
pub struct InventoryItem {
    widget: CustomWidget,
    is_selected: bool,
    item: ItemKind,
}

impl Control<CustomUiMessage, CustomUiNode> for InventoryItem {
    fn handle_routed_message(
        &mut self,
        ui: &mut UserInterface<CustomUiMessage, CustomUiNode>,
        message: &mut UiMessage<CustomUiMessage, CustomUiNode>,
    ) {
        self.widget.handle_routed_message(ui, message);
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum InventoryItemMessage {
    Select(bool),
}

impl Deref for InventoryItem {
    type Target = CustomWidget;

    fn deref(&self) -> &Self::Target {
        &self.widget
    }
}

impl DerefMut for InventoryItem {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.widget
    }
}

pub struct InventoryItemBuilder {
    widget_builder: UiWidgetBuilder,
}

impl InventoryItemBuilder {
    pub fn new(widget_builder: UiWidgetBuilder) -> Self {
        Self { widget_builder }
    }

    pub fn build(self, item: ItemKind, ctx: &mut BuildContext) -> Handle<UiNode> {
        let definition = Item::get_definition(item);

        let item = InventoryItem {
            widget: self
                .widget_builder
                .with_child(
                    BorderBuilder::new(
                        WidgetBuilder::new()
                            .with_foreground(Brush::Solid(Color::opaque(140, 140, 140)))
                            .with_child(
                                GridBuilder::new(
                                    WidgetBuilder::new()
                                        .with_child(
                                            ImageBuilder::new(WidgetBuilder::new().on_row(0))
                                                .build(ctx),
                                        )
                                        .with_child(
                                            TextBuilder::new(WidgetBuilder::new().on_row(1))
                                                .with_horizontal_text_alignment(
                                                    HorizontalAlignment::Center,
                                                )
                                                .with_vertical_text_alignment(
                                                    VerticalAlignment::Center,
                                                )
                                                .with_text(&definition.name)
                                                .build(ctx),
                                        ),
                                )
                                .add_row(Row::stretch())
                                .add_row(Row::strict(16.0))
                                .add_column(Column::stretch())
                                .build(ctx),
                            ),
                    )
                    .build(ctx),
                )
                .build(),
            is_selected: false,
            item,
        };

        ctx.add_node(UiNode::User(CustomUiNode::InventoryItem(item)))
    }
}

impl InventoryInterface {
    pub const WIDTH: f32 = 400.0;
    pub const HEIGHT: f32 = 300.0;

    pub fn new() -> Self {
        let mut ui = Gui::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let items_panel;
        BorderBuilder::new(
            WidgetBuilder::new()
                .with_foreground(Brush::Solid(Color::opaque(120, 120, 120)))
                .with_opacity(0.66)
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child(
                                TextBuilder::new(WidgetBuilder::new().on_row(0))
                                    .with_text("Inventory")
                                    .with_horizontal_text_alignment(HorizontalAlignment::Center)
                                    .build(&mut ui.build_ctx()),
                            )
                            .with_child(
                                GridBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(1)
                                        .with_child(
                                            ScrollViewerBuilder::new(WidgetBuilder::new())
                                                .with_content({
                                                    items_panel = WrapPanelBuilder::new(
                                                        WidgetBuilder::new()
                                                            .with_horizontal_alignment(
                                                                HorizontalAlignment::Left,
                                                            )
                                                            .with_vertical_alignment(
                                                                VerticalAlignment::Top,
                                                            )
                                                            .on_column(0),
                                                    )
                                                    .with_orientation(Orientation::Horizontal)
                                                    .build(&mut ui.build_ctx());
                                                    items_panel
                                                })
                                                .build(&mut ui.build_ctx()),
                                        )
                                        .with_child(
                                            TextBuilder::new(WidgetBuilder::new().on_column(1))
                                                .with_text("Description")
                                                .build(&mut ui.build_ctx()),
                                        ),
                                )
                                .add_column(Column::stretch())
                                .add_column(Column::strict(100.0))
                                .add_row(Row::stretch())
                                .build(&mut ui.build_ctx()),
                            ),
                    )
                    .add_row(Row::strict(30.0))
                    .add_row(Row::stretch())
                    .add_column(Column::stretch())
                    .build(&mut ui.build_ctx()),
                ),
        )
        .build(&mut ui.build_ctx());

        Self {
            ui,
            render_target,
            items_panel,
        }
    }

    pub fn sync_to_model(&mut self, player: &Player) {
        for &child in self.ui.node(self.items_panel).children() {
            self.ui
                .send_message(WidgetMessage::remove(child, MessageDirection::ToWidget));
        }

        for item in player.inventory().items() {
            let ctx = &mut self.ui.build_ctx();

            let widget = InventoryItemBuilder::new(
                WidgetBuilder::new()
                    .with_margin(Thickness::uniform(1.0))
                    .with_width(70.0)
                    .with_height(86.0),
            )
            .build(item.kind(), ctx);

            self.ui.send_message(WidgetMessage::link(
                widget,
                MessageDirection::ToWidget,
                self.items_panel,
            ));
        }
    }

    pub fn update(&mut self, delta: f32) {
        while self.ui.poll_message().is_some() {}

        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);
    }
}
