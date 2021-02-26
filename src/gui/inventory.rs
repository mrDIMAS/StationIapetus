use crate::{
    gui::{
        BuildContext, CustomUiMessage, CustomUiNode, CustomWidget, Gui, UiNode, UiWidgetBuilder,
    },
    item::ItemKind,
    player::Player,
};
use rg3d::{
    core::{algebra::Vector2, pool::Handle},
    gui::{
        border::BorderBuilder,
        grid::{Column, GridBuilder},
        message::{MessageDirection, UiMessage, WidgetMessage},
        scroll_viewer::ScrollViewerBuilder,
        widget::WidgetBuilder,
        wrap_panel::WrapPanelBuilder,
        Control, UserInterface,
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
        let item = InventoryItem {
            widget: self
                .widget_builder
                .with_child(BorderBuilder::new(WidgetBuilder::new()).build(ctx))
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
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new().with_child(
                            ScrollViewerBuilder::new(WidgetBuilder::new())
                                .with_content({
                                    items_panel = WrapPanelBuilder::new(WidgetBuilder::new())
                                        .build(&mut ui.build_ctx());
                                    items_panel
                                })
                                .build(&mut ui.build_ctx()),
                        ),
                    )
                    .add_column(Column::stretch())
                    .add_column(Column::strict(100.0))
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
        for item in player.inventory().items() {
            let ctx = &mut self.ui.build_ctx();

            let widget = InventoryItemBuilder::new(WidgetBuilder::new()).build(item.kind(), ctx);

            self.ui.send_message(WidgetMessage::link(
                self.items_panel,
                MessageDirection::ToWidget,
                widget,
            ))
        }
    }

    pub fn update(&mut self, delta: f32) {
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        while self.ui.poll_message().is_some() {}
    }
}
