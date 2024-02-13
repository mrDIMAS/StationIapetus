use crate::{
    character::{CharacterMessage, CharacterMessageData},
    control_scheme::{ControlButton, ControlScheme},
    gui,
    level::item::Item,
    message::Message,
    player::Player,
    MessageSender,
};
use fyrox::graph::BaseSceneGraph;
use fyrox::{
    core::{
        algebra::Vector2, color::Color, math, pool::Handle, reflect::prelude::*,
        type_traits::prelude::*, uuid_provider, visitor::prelude::*,
    },
    gui::{
        border::BorderBuilder,
        brush::Brush,
        define_constructor,
        draw::{CommandTexture, Draw, DrawingContext},
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        image::ImageBuilder,
        message::{ButtonState, MessageDirection, OsEvent, UiMessage},
        scroll_viewer::{ScrollViewerBuilder, ScrollViewerMessage},
        stack_panel::StackPanelBuilder,
        text::{TextBuilder, TextMessage},
        widget::{Widget, WidgetBuilder, WidgetMessage},
        wrap_panel::WrapPanelBuilder,
        BuildContext, Control, HorizontalAlignment, Orientation, Thickness, UiNode, UserInterface,
        VerticalAlignment,
    },
    resource::{model::ModelResource, texture::TextureResource},
    scene::node::Node,
};
use std::ops::{Deref, DerefMut};

pub struct InventoryInterface {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    items_panel: Handle<UiNode>,
    is_enabled: bool,
    sender: MessageSender,
    item_description: Handle<UiNode>,
    scroll_viewer: Handle<UiNode>,
}

#[derive(Debug, Clone, Reflect, Visit, ComponentProvider)]
pub struct InventoryItem {
    widget: Widget,
    is_selected: bool,
    item: ModelResource,
    #[allow(dead_code)]
    count: Handle<UiNode>,
}

uuid_provider!(InventoryItem = "346f2207-0868-4577-89a3-a4b36f3bf45d");

impl Control for InventoryItem {
    fn draw(&self, drawing_context: &mut DrawingContext) {
        let bounds = self.bounding_rect();
        drawing_context.push_rect(&bounds, 1.0);
        drawing_context.commit(
            self.clip_bounds(),
            self.foreground(),
            CommandTexture::None,
            None,
        );
    }

    fn handle_routed_message(&mut self, ui: &mut UserInterface, message: &mut UiMessage) {
        self.widget.handle_routed_message(ui, message);

        if let Some(&InventoryItemMessage::Select(select)) = message.data() {
            if message.destination() == self.handle() {
                self.is_selected = select;

                self.set_foreground(if select {
                    Brush::Solid(Color::opaque(0, 0, 255))
                } else {
                    Brush::Solid(Color::opaque(255, 255, 255))
                });
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InventoryItemMessage {
    Select(bool),
}

impl InventoryItemMessage {
    define_constructor!(InventoryItemMessage:Select => fn select(bool), layout: false);
}

impl Deref for InventoryItem {
    type Target = Widget;

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
    widget_builder: WidgetBuilder,
    count: usize,
}

impl InventoryItemBuilder {
    pub fn new(widget_builder: WidgetBuilder) -> Self {
        Self {
            widget_builder,
            count: 0,
        }
    }
    pub fn with_count(mut self, count: usize) -> Self {
        self.count = count;
        self
    }

    pub fn build(self, item_resource: &ModelResource, ctx: &mut BuildContext) -> Handle<UiNode> {
        let builder = self.widget_builder;
        Item::from_resource(item_resource, move |item| {
            if let Some(item) = item {
                let count;
                let body = BorderBuilder::new(
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
                                        .with_opt_texture(
                                            item.preview.deref().clone().map(Into::into),
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
                                                        .with_text((*item.name).clone())
                                                        .build(ctx),
                                                )
                                                .with_child({
                                                    count = TextBuilder::new(WidgetBuilder::new())
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
                .build(ctx);

                let item = InventoryItem {
                    widget: builder.with_child(body).build(),
                    count,
                    is_selected: false,
                    item: item_resource.clone(),
                };

                ctx.add_node(UiNode::new(item))
            } else {
                Default::default()
            }
        })
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

    pub fn new(sender: MessageSender) -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target = gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT);

        let items_panel;
        let item_description;
        let scroll_viewer;
        BorderBuilder::new(
            WidgetBuilder::new()
                .with_foreground(Brush::Solid(Color::opaque(120, 120, 120)))
                .with_opacity(Some(0.9))
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
                                                                        .with_wrap(WrapMode::Word)
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

            if let Some(resource) = item.resource.as_ref() {
                let widget = InventoryItemBuilder::new(
                    WidgetBuilder::new()
                        .with_margin(Thickness::uniform(1.0))
                        .with_width(70.0)
                        .with_height(100.0),
                )
                .with_count(item.amount as usize)
                .build(resource, ctx);

                self.ui.send_message(WidgetMessage::link(
                    widget,
                    MessageDirection::ToWidget,
                    self.items_panel,
                ));
            }
        }
    }

    pub fn selection(&self) -> Handle<UiNode> {
        for &item_handle in self.ui.node(self.items_panel).children() {
            if let Some(inventory_item) = self.ui.node(item_handle).cast::<InventoryItem>() {
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
                self.ui.send_message(InventoryItemMessage::select(
                    closest,
                    MessageDirection::ToWidget,
                    true,
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
        player: &mut Player,
        player_handle: Handle<Node>,
    ) {
        self.ui.process_os_event(os_event);

        if self.is_enabled {
            if let OsEvent::KeyboardInput { button, state, .. } = *os_event {
                if state == ButtonState::Pressed {
                    // TODO: Add support for other input bindings.
                    if let ControlButton::Key(key) = control_scheme.cursor_up.button {
                        if fyrox::utils::translate_key_to_ui(key) == button {
                            self.try_move_selection(MoveDirection::Up);
                        }
                    }
                    if let ControlButton::Key(key) = control_scheme.cursor_down.button {
                        if fyrox::utils::translate_key_to_ui(key) == button {
                            self.try_move_selection(MoveDirection::Down);
                        }
                    }
                    if let ControlButton::Key(key) = control_scheme.cursor_left.button {
                        if fyrox::utils::translate_key_to_ui(key) == button {
                            self.try_move_selection(MoveDirection::Left);
                        }
                    }
                    if let ControlButton::Key(key) = control_scheme.cursor_right.button {
                        if fyrox::utils::translate_key_to_ui(key) == button {
                            self.try_move_selection(MoveDirection::Right);
                        }
                    }
                    if let ControlButton::Key(key) = control_scheme.action.button {
                        if fyrox::utils::translate_key_to_ui(key) == button {
                            let selection = self.selection();
                            if selection.is_some() {
                                if let Some(item) = self.ui.node(selection).cast::<InventoryItem>()
                                {
                                    let item_resource = &item.item;
                                    Item::from_resource(item_resource, |item| {
                                        if let Some(item) = item {
                                            if item.enabled {
                                                if *item.consumable
                                                    && player
                                                        .inventory_mut()
                                                        .try_extract_exact_items(item_resource, 1)
                                                        == 1
                                                {
                                                    player.use_item(item);
                                                    self.sender.send(Message::SyncInventory);
                                                }

                                                player
                                                    .script_message_sender
                                                    .as_ref()
                                                    .unwrap()
                                                    .send_to_target(
                                                        player_handle,
                                                        CharacterMessage {
                                                            character: player_handle,
                                                            data:
                                                                CharacterMessageData::SelectWeapon(
                                                                    item_resource.clone(),
                                                                ),
                                                        },
                                                    );
                                            }
                                        }
                                    });
                                } else {
                                    unreachable!()
                                }
                            }
                        }
                    }
                    if let ControlButton::Key(key) = control_scheme.drop_item.button {
                        if fyrox::utils::translate_key_to_ui(key) == button {
                            let selection = self.selection();
                            if selection.is_some() {
                                if let Some(item) = self.ui.node(selection).cast::<InventoryItem>()
                                {
                                    player
                                        .script_message_sender
                                        .as_ref()
                                        .unwrap()
                                        .send_to_target(
                                            player_handle,
                                            CharacterMessage {
                                                character: player_handle,
                                                data: CharacterMessageData::DropItems {
                                                    item: item.item.clone(),
                                                    count: 1,
                                                },
                                            },
                                        );
                                    self.sender.send(Message::SyncInventory);
                                } else {
                                    unreachable!()
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn update(&mut self, delta: f32) {
        while let Some(message) = self.ui.poll_message() {
            if let Some(&InventoryItemMessage::Select(select)) = message.data() {
                if select {
                    if let Some(item) = self.ui.node(message.destination()).cast::<InventoryItem>()
                    {
                        // Deselect every other item.
                        for &item_handle in self.ui.node(self.items_panel).children() {
                            if item_handle != message.destination() {
                                self.ui.send_message(InventoryItemMessage::select(
                                    item_handle,
                                    MessageDirection::ToWidget,
                                    false,
                                ));
                            }
                        }

                        Item::from_resource(&item.item, |item| {
                            if let Some(item) = item {
                                self.ui.send_message(TextMessage::text(
                                    self.item_description,
                                    MessageDirection::ToWidget,
                                    item.description.deref().clone(),
                                ));
                            }
                        });
                    } else {
                        unreachable!();
                    }
                }
            }
        }

        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);
    }
}
