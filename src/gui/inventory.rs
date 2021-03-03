use crate::{
    actor::Actor,
    control_scheme::{ControlButton, ControlScheme},
    gui::{
        BuildContext, CustomUiMessage, CustomUiNode, CustomWidget, Gui, UiNode, UiWidgetBuilder,
    },
    item::{Item, ItemKind},
    message::Message,
    player::Player,
};
use rg3d::{
    core::{algebra::Vector2, color::Color, math, pool::Handle},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        draw::{CommandTexture, Draw, DrawingContext},
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        message::{
            ButtonState, MessageDirection, OsEvent, ScrollViewerMessage, TextMessage, UiMessage,
            UiMessageData, WidgetMessage,
        },
        scroll_viewer::ScrollViewerBuilder,
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::WidgetBuilder,
        wrap_panel::WrapPanelBuilder,
        Control, HorizontalAlignment, Orientation, Thickness, UserInterface, VerticalAlignment,
    },
    resource::texture::Texture,
    scene::graph::Graph,
};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Sender,
};

pub struct InventoryInterface {
    pub ui: Gui,
    pub render_target: Texture,
    items_panel: Handle<UiNode>,
    is_enabled: bool,
    sender: Sender<Message>,
    item_description: Handle<UiNode>,
    scroll_viewer: Handle<UiNode>,
}

#[derive(Debug, Clone)]
pub struct InventoryItem {
    widget: CustomWidget,
    is_selected: bool,
    item: ItemKind,
    count: Handle<UiNode>,
}

impl Control<CustomUiMessage, CustomUiNode> for InventoryItem {
    fn draw(&self, drawing_context: &mut DrawingContext) {
        let bounds = self.screen_bounds();
        drawing_context.push_rect(&bounds, 1.0);
        drawing_context.commit(bounds, self.foreground(), CommandTexture::None, None);
    }

    fn handle_routed_message(
        &mut self,
        ui: &mut UserInterface<CustomUiMessage, CustomUiNode>,
        message: &mut UiMessage<CustomUiMessage, CustomUiNode>,
    ) {
        self.widget.handle_routed_message(ui, message);

        match message.data() {
            UiMessageData::User(msg) => {
                let CustomUiMessage::InventoryItem(InventoryItemMessage::Select(select)) = *msg;
                if message.destination() == self.handle() {
                    self.is_selected = select;

                    self.set_foreground(if select {
                        Brush::Solid(Color::opaque(0, 0, 255))
                    } else {
                        Brush::Solid(Color::opaque(255, 255, 255))
                    });
                }
            }
            _ => (),
        }
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
    count: usize,
}

impl InventoryItemBuilder {
    pub fn new(widget_builder: UiWidgetBuilder) -> Self {
        Self {
            widget_builder,
            count: 0,
        }
    }
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn build(self, item: ItemKind, ctx: &mut BuildContext) -> Handle<UiNode> {
        let definition = Item::get_definition(item);

        let count;
        let item =
            InventoryItem {
                widget: self
                    .widget_builder
                    .with_child(
                        BorderBuilder::new(
                            WidgetBuilder::new()
                                .with_margin(Thickness::uniform(1.0))
                                .with_foreground(Brush::Solid(Color::opaque(140, 140, 140)))
                                .with_child(
                                    GridBuilder::new(
                                        WidgetBuilder::new()
                                            .with_child(
                                                ImageBuilder::new(
                                                    WidgetBuilder::new()
                                                        .with_margin(Thickness::uniform(1.0))
                                                        .on_row(0),
                                                )
                                                .build(ctx),
                                            )
                                            .with_child(
                                                StackPanelBuilder::new(
                                                    WidgetBuilder::new()
                                                        .on_row(1)
                                                        .with_child(
                                                            TextBuilder::new(WidgetBuilder::new())
                                                                .with_horizontal_text_alignment(
                                                                    HorizontalAlignment::Center,
                                                                )
                                                                .with_vertical_text_alignment(
                                                                    VerticalAlignment::Center,
                                                                )
                                                                .with_text(&definition.name)
                                                                .build(ctx),
                                                        )
                                                        .with_child({
                                                            count = TextBuilder::new(
                                                                WidgetBuilder::new(),
                                                            )
                                                            .with_horizontal_text_alignment(
                                                                HorizontalAlignment::Center,
                                                            )
                                                            .with_vertical_text_alignment(
                                                                VerticalAlignment::Center,
                                                            )
                                                            .with_text(format!("x{}", self.count))
                                                            .build(ctx);
                                                            count
                                                        }),
                                                )
                                                .build(ctx),
                                            ),
                                    )
                                    .add_row(Row::stretch())
                                    .add_row(Row::auto())
                                    .add_column(Column::stretch())
                                    .build(ctx),
                                ),
                        )
                        .build(ctx),
                    )
                    .build(),
                count,
                is_selected: false,
                item,
            };

        ctx.add_node(UiNode::User(CustomUiNode::InventoryItem(item)))
    }
}

enum MoveDirection {
    Up,
    Down,
    Left,
    Right,
}

impl InventoryInterface {
    pub const WIDTH: f32 = 400.0;
    pub const HEIGHT: f32 = 300.0;

    pub fn new(sender: Sender<Message>) -> Self {
        let mut ui = Gui::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = Texture::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let items_panel;
        let item_description;
        let scroll_viewer;
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
                                        .with_child({
                                            scroll_viewer =
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
                                                    .build(&mut ui.build_ctx());
                                            scroll_viewer
                                        })
                                        .with_child(
                                            BorderBuilder::new(
                                                WidgetBuilder::new()
                                                    .on_column(1)
                                                    .with_background(Brush::Solid(Color::opaque(
                                                        80, 80, 80,
                                                    )))
                                                    .with_child(
                                                        StackPanelBuilder::new(
                                                            WidgetBuilder::new()
                                                                .with_child(
                                                                    TextBuilder::new(
                                                                        WidgetBuilder::new(),
                                                                    )
                                                                    .with_text("Item")
                                                                    .with_horizontal_text_alignment(
                                                                        HorizontalAlignment::Center,
                                                                    )
                                                                    .build(&mut ui.build_ctx()),
                                                                )
                                                                .with_child({
                                                                    item_description =
                                                                        TextBuilder::new(
                                                                            WidgetBuilder::new(),
                                                                        )
                                                                        .with_wrap(true)
                                                                        .build(&mut ui.build_ctx());
                                                                    item_description
                                                                }),
                                                        )
                                                        .build(&mut ui.build_ctx()),
                                                    ),
                                            )
                                            .build(&mut ui.build_ctx()),
                                        ),
                                )
                                .add_column(Column::stretch())
                                .add_column(Column::strict(150.0))
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
            is_enabled: true,
            sender,
            item_description,
            scroll_viewer,
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
                    .with_height(100.0),
            )
            .with_count(item.amount() as usize)
            .build(item.kind(), ctx);

            self.ui.send_message(WidgetMessage::link(
                widget,
                MessageDirection::ToWidget,
                self.items_panel,
            ));
        }
    }

    pub fn selection(&self) -> Handle<UiNode> {
        for &item_handle in self.ui.node(self.items_panel).children() {
            if let UiNode::User(CustomUiNode::InventoryItem(inventory_item)) =
                self.ui.node(item_handle)
            {
                if inventory_item.is_selected {
                    return item_handle;
                }
            }
        }
        Handle::NONE
    }

    fn try_move_selection(&mut self, dir: MoveDirection) {
        let items = self.ui.node(self.items_panel).children();

        let mut direction = match dir {
            MoveDirection::Up => -Vector2::y(),
            MoveDirection::Down => Vector2::y(),
            MoveDirection::Left => -Vector2::x(),
            MoveDirection::Right => Vector2::x(),
        }
        .scale(999.0);

        direction += Vector2::new(std::f32::EPSILON, std::f32::EPSILON);

        if !items.is_empty() {
            let current_selection = self.selection();

            let current_bounds = if current_selection.is_some() {
                self.ui.node(current_selection).screen_bounds()
            } else {
                self.ui.node(*items.first().unwrap()).screen_bounds()
            };
            let origin = (current_bounds.left_top_corner() + current_bounds.right_bottom_corner())
                .scale(0.5);

            let mut closest = Handle::NONE;
            let mut closest_distance = std::f32::MAX;

            for &item_handle in items {
                let item_bounds = self.ui.node(item_handle).screen_bounds();

                if let Some(intersection) =
                    math::ray_rect_intersection(item_bounds, origin, direction)
                {
                    if intersection.min < closest_distance && item_handle != current_selection {
                        closest_distance = intersection.min;
                        closest = item_handle;
                    }
                }
            }

            if closest.is_some() {
                self.ui.send_message(UiMessage::user(
                    closest,
                    MessageDirection::ToWidget,
                    CustomUiMessage::InventoryItem(InventoryItemMessage::Select(true)),
                ));

                self.ui.send_message(ScrollViewerMessage::bring_into_view(
                    self.scroll_viewer,
                    MessageDirection::ToWidget,
                    closest,
                ));
            }
        }
    }

    pub fn process_os_event(
        &mut self,
        os_event: &OsEvent,
        control_scheme: &ControlScheme,
        player_handle: Handle<Actor>,
        player: &mut Player,
        graph: &Graph,
    ) {
        self.ui.process_os_event(os_event);

        if self.is_enabled {
            match *os_event {
                OsEvent::KeyboardInput { button, state } => {
                    if state == ButtonState::Pressed {
                        // TODO: Add support for other input bindings.
                        if let ControlButton::Key(key) = control_scheme.cursor_up.button {
                            if rg3d::utils::translate_key(key) == button {
                                self.try_move_selection(MoveDirection::Up);
                            }
                        }
                        if let ControlButton::Key(key) = control_scheme.cursor_down.button {
                            if rg3d::utils::translate_key(key) == button {
                                self.try_move_selection(MoveDirection::Down);
                            }
                        }
                        if let ControlButton::Key(key) = control_scheme.cursor_left.button {
                            if rg3d::utils::translate_key(key) == button {
                                self.try_move_selection(MoveDirection::Left);
                            }
                        }
                        if let ControlButton::Key(key) = control_scheme.cursor_right.button {
                            if rg3d::utils::translate_key(key) == button {
                                self.try_move_selection(MoveDirection::Right);
                            }
                        }
                        if let ControlButton::Key(key) = control_scheme.action.button {
                            if rg3d::utils::translate_key(key) == button {
                                let selection = self.selection();
                                if selection.is_some() {
                                    if let UiNode::User(CustomUiNode::InventoryItem(item)) =
                                        self.ui.node(selection)
                                    {
                                        let definition = Item::get_definition(item.item);
                                        if definition.consumable
                                            && player
                                                .inventory_mut()
                                                .try_extract_exact_items(item.item, 1)
                                                == 1
                                        {
                                            self.sender
                                                .send(Message::GiveItem {
                                                    actor: player_handle,
                                                    kind: item.item,
                                                })
                                                .unwrap();
                                            self.sender.send(Message::SyncInventory).unwrap();
                                        }
                                    } else {
                                        unreachable!()
                                    }
                                }
                            }
                        }
                        if let ControlButton::Key(key) = control_scheme.drop_item.button {
                            if rg3d::utils::translate_key(key) == button {
                                let selection = self.selection();
                                if selection.is_some() {
                                    if let UiNode::User(CustomUiNode::InventoryItem(item)) =
                                        self.ui.node(selection)
                                    {
                                        let definition = Item::get_definition(item.item);
                                        if player
                                            .inventory_mut()
                                            .try_extract_exact_items(item.item, 1)
                                            == 1
                                        {
                                            self.sender
                                                .send(Message::SpawnItem {
                                                    kind: item.item,
                                                    position: player.position(graph),
                                                    adjust_height: true,
                                                })
                                                .unwrap();
                                            self.sender.send(Message::SyncInventory).unwrap();
                                        }
                                    } else {
                                        unreachable!()
                                    }
                                }
                            }
                        }
                    }
                }
                _ => (),
            }
        }
    }

    pub fn update(&mut self, delta: f32) {
        while let Some(message) = self.ui.poll_message() {
            match message.data() {
                UiMessageData::User(msg) => {
                    let CustomUiMessage::InventoryItem(InventoryItemMessage::Select(select)) = *msg;

                    if select {
                        if let UiNode::User(CustomUiNode::InventoryItem(item)) =
                            self.ui.node(message.destination())
                        {
                            let definition = Item::get_definition(item.item);

                            // Deselect every other item.
                            for &item_handle in self.ui.node(self.items_panel).children() {
                                if item_handle != message.destination() {
                                    self.ui.send_message(UiMessage::user(
                                        item_handle,
                                        MessageDirection::ToWidget,
                                        CustomUiMessage::InventoryItem(
                                            InventoryItemMessage::Select(false),
                                        ),
                                    ));
                                }
                            }

                            self.ui.send_message(TextMessage::text(
                                self.item_description,
                                MessageDirection::ToWidget,
                                definition.description.clone(),
                            ));
                        } else {
                            unreachable!();
                        }
                    }
                }
                _ => (),
            }
        }

        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);
    }
}
