//! Contains all helper functions that creates styled widgets for game user interface.
//! However most of the styles are used from dark theme of rg3d-ui library so there
//! is not much.

use crate::{message::Message, MessageSender};
use fyrox::{
    core::{pool::Handle, visitor::prelude::*},
    graph::BaseSceneGraph,
    gui::{
        border::BorderBuilder,
        brush::Brush,
        button::{ButtonBuilder, ButtonMessage},
        check_box::CheckBoxBuilder,
        core::color::Color,
        font::FontResource,
        grid::{Column, GridBuilder, Row},
        message::{MessageDirection, UiMessage},
        scroll_bar::ScrollBarBuilder,
        stack_panel::StackPanelBuilder,
        text::TextBuilder,
        widget::{WidgetBuilder, WidgetMessage},
        BuildContext, HorizontalAlignment, Orientation, Thickness, UiNode, UserInterface,
        VerticalAlignment,
    },
    resource::texture::{TextureResource, TextureResourceExtension, TextureWrapMode},
};

pub mod inventory;
pub mod item_display;
pub mod journal;
pub mod weapon_display;

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

#[derive(Visit, Default)]
pub struct DeathScreen {
    pub root: Handle<UiNode>,
    load_game: Handle<UiNode>,
    exit_to_menu: Handle<UiNode>,
    exit_game: Handle<UiNode>,
}

impl DeathScreen {
    pub fn new(ui: &mut UserInterface, font: FontResource) -> Self {
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
                                .with_font_size(31.0)
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
                                            .with_text_and_font("Load Game", font.clone())
                                            .build(&mut ui.build_ctx());
                                            load_game
                                        })
                                        .with_child({
                                            exit_to_menu = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_margin(Thickness::uniform(2.0)),
                                            )
                                            .with_text_and_font("Exit To Menu", font.clone())
                                            .build(&mut ui.build_ctx());
                                            exit_to_menu
                                        })
                                        .with_child({
                                            exit_game = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_margin(Thickness::uniform(2.0)),
                                            )
                                            .with_text_and_font("Exit Game", font)
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
        }
    }

    pub fn resize(&self, ui: &UserInterface, width: f32, height: f32) {
        ui.send_message(WidgetMessage::width(
            self.root,
            MessageDirection::ToWidget,
            width,
        ));
        ui.send_message(WidgetMessage::height(
            self.root,
            MessageDirection::ToWidget,
            height,
        ));
    }

    pub fn handle_ui_message(&mut self, message: &UiMessage, sender: &MessageSender) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.load_game {
                sender.send(Message::LoadGame);
            } else if message.destination() == self.exit_to_menu {
                sender.send(Message::ToggleMainMenu);
            } else if message.destination() == self.exit_game {
                sender.send(Message::QuitGame);
            }
        }
    }

    pub fn set_visible(&self, ui: &UserInterface, state: bool) {
        ui.send_message(WidgetMessage::visibility(
            self.root,
            MessageDirection::ToWidget,
            state,
        ));
    }

    pub fn is_visible(&self, ui: &UserInterface) -> bool {
        ui.node(self.root).visibility()
    }
}

#[derive(Visit, Default)]
pub struct FinalScreen {
    root: Handle<UiNode>,
    exit_to_menu: Handle<UiNode>,
    exit_game: Handle<UiNode>,
}

impl FinalScreen {
    pub fn new(ui: &mut UserInterface, font: FontResource) -> Self {
        let exit_to_menu;
        let exit_game;
        let root = BorderBuilder::new(
            WidgetBuilder::new()
                .with_visibility(false)
                .with_width(ui.screen_size().x)
                .with_height(ui.screen_size().y)
                .with_background(Brush::Solid(Color::opaque(40, 40, 40)))
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child(
                                TextBuilder::new(
                                    WidgetBuilder::new()
                                        .with_foreground(Brush::Solid(Color::opaque(255, 255, 255)))
                                        .on_row(0)
                                        .on_column(0)
                                        .with_horizontal_alignment(HorizontalAlignment::Center)
                                        .with_vertical_alignment(VerticalAlignment::Bottom),
                                )
                                .with_text("Thanks for playing demo version of the game!")
                                .with_font(font.clone())
                                .with_font_size(31.0)
                                .build(&mut ui.build_ctx()),
                            )
                            .with_child(
                                StackPanelBuilder::new(
                                    WidgetBuilder::new()
                                        .with_vertical_alignment(VerticalAlignment::Top)
                                        .on_row(1)
                                        .on_column(0)
                                        .with_child({
                                            exit_to_menu = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_width(300.0)
                                                    .with_margin(Thickness::uniform(2.0)),
                                            )
                                            .with_text_and_font("Exit To Menu", font.clone())
                                            .build(&mut ui.build_ctx());
                                            exit_to_menu
                                        })
                                        .with_child({
                                            exit_game = ButtonBuilder::new(
                                                WidgetBuilder::new()
                                                    .with_margin(Thickness::uniform(2.0))
                                                    .with_width(300.0),
                                            )
                                            .with_text_and_font("Exit Game", font)
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
                    .build(&mut ui.build_ctx()),
                ),
        )
        .build(&mut ui.build_ctx());

        Self {
            root,
            exit_to_menu,
            exit_game,
        }
    }

    pub fn handle_ui_message(&mut self, message: &UiMessage, sender: &MessageSender) {
        if let Some(ButtonMessage::Click) = message.data() {
            if message.destination() == self.exit_to_menu {
                sender.send(Message::ToggleMainMenu);
            } else if message.destination() == self.exit_game {
                sender.send(Message::QuitGame);
            }
        }
    }

    pub fn set_visible(&self, ui: &UserInterface, state: bool) {
        ui.send_message(WidgetMessage::visibility(
            self.root,
            MessageDirection::ToWidget,
            state,
        ));
    }

    pub fn is_visible(&self, ui: &UserInterface) -> bool {
        ui.node(self.root).visibility()
    }
}

pub fn create_ui_render_target(width: f32, height: f32) -> TextureResource {
    let render_target = TextureResource::new_render_target(width as u32, height as u32);
    let mut texture = render_target.data_ref();
    texture.set_s_wrap_mode(TextureWrapMode::ClampToEdge);
    texture.set_t_wrap_mode(TextureWrapMode::ClampToEdge);
    drop(texture);
    render_target
}
