//! Contains all helper functions that creates styled widgets for game user interface.
//! However most of the styles are used from dark theme of rg3d-ui library so there
//! is not much.

use crate::{
    gui::inventory::{InventoryItem, InventoryItemMessage},
    message::Message,
};
use rg3d::{
    core::{algebra::Vector2, pool::Handle},
    gui::{
        border::BorderBuilder,
        brush::Brush,
        button::ButtonBuilder,
        check_box::CheckBoxBuilder,
        core::color::Color,
        draw::DrawingContext,
        grid::{Column, GridBuilder, Row},
        message::{
            ButtonMessage, MessageData, MessageDirection, OsEvent, UiMessage, UiMessageData,
            WidgetMessage,
        },
        node::UINode,
        scroll_bar::ScrollBarBuilder,
        scroll_viewer::ScrollViewerBuilder,
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        ttf::SharedFont,
        widget::WidgetBuilder,
        Control, HorizontalAlignment, NodeHandleMapping, Orientation, Thickness, UserInterface,
        VerticalAlignment,
    },
};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Sender,
};

pub mod inventory;
pub mod item_display;
pub mod weapon_display;

#[derive(Debug, Clone)]
pub enum CustomUiNode {
    InventoryItem(InventoryItem),
}

macro_rules! static_dispatch {
    ($self:ident, $func:ident, $($args:expr),*) => {
        match $self {
            CustomUiNode::InventoryItem(v) => v.$func($($args),*),
        }
    }
}

impl Deref for CustomUiNode {
    type Target = CustomWidget;

    fn deref(&self) -> &Self::Target {
        static_dispatch!(self, deref,)
    }
}

impl DerefMut for CustomUiNode {
    fn deref_mut(&mut self) -> &mut Self::Target {
        static_dispatch!(self, deref_mut,)
    }
}

impl Control<CustomUiMessage, CustomUiNode> for CustomUiNode {
    fn resolve(&mut self, node_map: &NodeHandleMapping<CustomUiMessage, CustomUiNode>) {
        static_dispatch!(self, resolve, node_map);
    }

    fn measure_override(&self, ui: &Gui, available_size: Vector2<f32>) -> Vector2<f32> {
        static_dispatch!(self, measure_override, ui, available_size)
    }

    fn arrange_override(&self, ui: &Gui, final_size: Vector2<f32>) -> Vector2<f32> {
        static_dispatch!(self, arrange_override, ui, final_size)
    }

    fn draw(&self, drawing_context: &mut DrawingContext) {
        static_dispatch!(self, draw, drawing_context)
    }

    fn update(&mut self, dt: f32) {
        static_dispatch!(self, update, dt)
    }

    fn handle_routed_message(&mut self, ui: &mut Gui, message: &mut GuiMessage) {
        static_dispatch!(self, handle_routed_message, ui, message)
    }

    fn preview_message(&self, ui: &Gui, message: &mut GuiMessage) {
        static_dispatch!(self, preview_message, ui, message)
    }

    fn handle_os_event(&mut self, self_handle: Handle<UiNode>, ui: &mut Gui, event: &OsEvent) {
        static_dispatch!(self, handle_os_event, self_handle, ui, event)
    }

    fn remove_ref(&mut self, handle: Handle<UiNode>) {
        static_dispatch!(self, remove_ref, handle)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum CustomUiMessage {
    InventoryItem(InventoryItemMessage),
}

impl MessageData for CustomUiMessage {}

pub type UiNode = UINode<CustomUiMessage, CustomUiNode>;
pub type UiNodeHandle = Handle<UiNode>;
pub type Gui = UserInterface<CustomUiMessage, CustomUiNode>;
pub type GuiMessage = UiMessage<CustomUiMessage, CustomUiNode>;
pub type BuildContext<'a> = rg3d::gui::BuildContext<'a, CustomUiMessage, CustomUiNode>;
pub type CustomWidget = rg3d::gui::widget::Widget<CustomUiMessage, CustomUiNode>;
pub type UiWidgetBuilder = rg3d::gui::widget::WidgetBuilder<CustomUiMessage, CustomUiNode>;

pub struct ScrollBarData {
    pub min: f32,
    pub max: f32,
    pub value: f32,
    pub step: f32,
    pub row: usize,
    pub column: usize,
    pub margin: Thickness,
    pub show_value: bool,
    pub orientation: Orientation,
}

pub fn create_scroll_bar(ctx: &mut BuildContext, data: ScrollBarData) -> Handle<UiNode> {
    let mut wb = WidgetBuilder::new();
    match data.orientation {
        Orientation::Vertical => wb = wb.with_width(30.0),
        Orientation::Horizontal => wb = wb.with_height(30.0),
    }
    ScrollBarBuilder::new(
        wb.on_row(data.row)
            .on_column(data.column)
            .with_margin(data.margin),
    )
    .with_orientation(data.orientation)
    .show_value(data.show_value)
    .with_max(data.max)
    .with_min(data.min)
    .with_step(data.step)
    .with_value(data.value)
    .with_value_precision(1)
    .build(ctx)
}

pub fn create_check_box(
    ctx: &mut BuildContext,
    row: usize,
    column: usize,
    checked: bool,
) -> Handle<UiNode> {
    CheckBoxBuilder::new(
        WidgetBuilder::new()
            .with_margin(Thickness::uniform(2.0))
            .with_width(24.0)
            .with_height(24.0)
            .on_row(row)
            .on_column(column)
            .with_vertical_alignment(VerticalAlignment::Center)
            .with_horizontal_alignment(HorizontalAlignment::Left),
    )
    .checked(Some(checked))
    .build(ctx)
}

pub fn create_scroll_viewer(ctx: &mut BuildContext) -> Handle<UiNode> {
    ScrollViewerBuilder::new(WidgetBuilder::new()).build(ctx)
}

pub struct DeathScreen {
    root: Handle<UiNode>,
    load_game: Handle<UiNode>,
    exit_to_menu: Handle<UiNode>,
    exit_game: Handle<UiNode>,
    sender: Sender<Message>,
}

impl DeathScreen {
    pub fn new(ui: &mut Gui, font: SharedFont, sender: Sender<Message>) -> Self {
        let load_game;
        let exit_to_menu;
        let exit_game;
        let root = BorderBuilder::new(
            WidgetBuilder::new()
                .with_visibility(false)
                .with_width(ui.screen_size().x)
                .with_height(ui.screen_size().y)
                .with_background(Brush::Solid(Color::opaque(30, 0, 0)))
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .with_foreground(Brush::Solid(Color::opaque(255, 0, 0)))
                                        .on_row(0)
                                        .on_column(1)
                                        .with_horizontal_alignment(HorizontalAlignment::Center)
                                        .with_vertical_alignment(VerticalAlignment::Bottom),
                                )
                                .with_text("You Died")
                                .with_font(font.clone())
                                .build(&mut ui.build_ctx()),
                            )
                            .with_child(
                                StackPanelBuilder::new(
                                    WidgetBuilder::new()
                                        .with_vertical_alignment(VerticalAlignment::Top)
                                        .on_row(1)
                                        .on_column(1)
                                        .with_child({
                                            load_game = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_margin(Thickness::uniform(2.0)),
                                            )
                                            .with_text("Load Game")
                                            .with_font(font.clone())
                                            .build(&mut ui.build_ctx());
                                            load_game
                                        })
                                        .with_child({
                                            exit_to_menu = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_margin(Thickness::uniform(2.0)),
                                            )
                                            .with_text("Exit To Menu")
                                            .with_font(font.clone())
                                            .build(&mut ui.build_ctx());
                                            exit_to_menu
                                        })
                                        .with_child({
                                            exit_game = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_margin(Thickness::uniform(2.0)),
                                            )
                                            .with_text("Exit Game")
                                            .with_font(font)
                                            .build(&mut ui.build_ctx());
                                            exit_game
                                        }),
                                )
                                .build(&mut ui.build_ctx()),
                            ),
                    )
                    .add_row(Row::stretch())
                    .add_row(Row::stretch())
                    .add_column(Column::stretch())
                    .add_column(Column::strict(300.0))
                    .add_column(Column::stretch())
                    .build(&mut ui.build_ctx()),
                ),
        )
        .build(&mut ui.build_ctx());

        Self {
            root,
            load_game,
            exit_to_menu,
            exit_game,
            sender,
        }
    }

    pub fn handle_ui_message(&mut self, message: &GuiMessage) {
        if let UiMessageData::Button(ButtonMessage::Click) = message.data() {
            if message.destination() == self.load_game {
                self.sender.send(Message::LoadGame).unwrap();
            } else if message.destination() == self.exit_to_menu {
                self.sender.send(Message::ToggleMainMenu).unwrap();
            } else if message.destination() == self.exit_game {
                self.sender.send(Message::QuitGame).unwrap();
            }
        }
    }

    pub fn set_visible(&self, ui: &Gui, state: bool) {
        ui.send_message(WidgetMessage::visibility(
            self.root,
            MessageDirection::ToWidget,
            state,
        ));
    }

    pub fn is_visible(&self, ui: &Gui) -> bool {
        ui.node(self.root).visibility()
    }
}
