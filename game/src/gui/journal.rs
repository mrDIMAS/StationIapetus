#![allow(dead_code)] // TODO

use crate::control_scheme::{ControlButton, ControlScheme};
use fyrox::resource::texture::{TextureResource, TextureResourceExtension};
use fyrox::{
    core::{
        algebra::Vector2,
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    gui::{
        border::BorderBuilder,
        decorator::DecoratorBuilder,
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        list_view::{ListViewBuilder, ListViewMessage},
        message::{ButtonState, MessageDirection, OsEvent},
        scroll_viewer::ScrollViewerBuilder,
        text::{TextBuilder, TextMessage},
        widget::WidgetBuilder,
        UiNode, UserInterface,
    },
    lazy_static::lazy_static,
    resource::texture::Texture,
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File};

#[derive(Deserialize, Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Hash, Visit, Debug)]
#[repr(u32)]
pub enum JournalEntryKind {
    CurrentSituation,
}

impl Default for JournalEntryKind {
    fn default() -> Self {
        Self::CurrentSituation
    }
}

#[derive(Deserialize)]
pub struct JournalEntryDefinition {
    pub title: String,
    pub text: String,
}

#[derive(Deserialize, Default)]
pub struct JournalEntryDefinitionContainer {
    map: HashMap<JournalEntryKind, JournalEntryDefinition>,
}

impl JournalEntryDefinitionContainer {
    pub fn new() -> Self {
        let file = File::open("data/configs/journal.ron").unwrap();
        ron::de::from_reader(file).unwrap()
    }
}

lazy_static! {
    static ref DEFINITIONS: JournalEntryDefinitionContainer =
        JournalEntryDefinitionContainer::new();
}

impl JournalEntryKind {
    pub fn get_definition(self) -> &'static JournalEntryDefinition {
        DEFINITIONS.map.get(&self).unwrap()
    }
}

#[derive(Default, Visit, Debug)]
pub struct Journal {
    messages: Vec<JournalEntryKind>,
}

impl Journal {
    pub fn new() -> Self {
        Self {
            messages: vec![JournalEntryKind::CurrentSituation],
        }
    }
}

pub struct JournalDisplay {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    objective: Handle<UiNode>,
    messages: Handle<UiNode>,
    message_text: Handle<UiNode>,
    current_message: Option<usize>,
}

impl Default for JournalDisplay {
    fn default() -> Self {
        Self::new()
    }
}

impl JournalDisplay {
    pub const WIDTH: f32 = 400.0;
    pub const HEIGHT: f32 = 300.0;

    pub fn new() -> Self {
        let mut ui = UserInterface::new(Vector2::new(Self::WIDTH, Self::HEIGHT));

        let render_target =
            TextureResource::new_render_target(Self::WIDTH as u32, Self::HEIGHT as u32);

        let objective;
        let messages;
        let message_text;
        BorderBuilder::new(
            WidgetBuilder::new()
                .with_width(Self::WIDTH)
                .with_height(Self::HEIGHT)
                .with_child(
                    GridBuilder::new(
                        WidgetBuilder::new()
                            .with_child({
                                objective =
                                    TextBuilder::new(WidgetBuilder::new().on_row(0).on_column(0))
                                        .with_text("Investigate the reasons why connection with the station was lost.")
                                        .with_wrap(WrapMode::Word)
                                        .build(&mut ui.build_ctx());
                                objective
                            })
                            .with_child(
                                GridBuilder::new(
                                    WidgetBuilder::new()
                                        .on_row(1)
                                        .on_column(0)
                                        .with_child({
                                            messages = ListViewBuilder::new(
                                                WidgetBuilder::new().on_column(0).on_row(0),
                                            )
                                            .build(&mut ui.build_ctx());
                                            messages
                                        })
                                        .with_child(
                                            ScrollViewerBuilder::new(
                                                WidgetBuilder::new().on_column(1).on_row(0),
                                            )
                                            .with_content({
                                                message_text =
                                                    TextBuilder::new(WidgetBuilder::new())
                                                        .with_wrap(WrapMode::Word)
                                                        .build(&mut ui.build_ctx());
                                                message_text
                                            })
                                            .build(&mut ui.build_ctx()),
                                        ),
                                )
                                .add_row(Row::stretch())
                                .add_column(Column::strict(150.0))
                                .add_column(Column::stretch())
                                .build(&mut ui.build_ctx()),
                            ),
                    )
                    .add_row(Row::strict(60.0))
                    .add_row(Row::stretch())
                    .add_column(Column::stretch())
                    .build(&mut ui.build_ctx()),
                ),
        )
        .build(&mut ui.build_ctx());

        Self {
            current_message: None,
            ui,
            render_target,
            objective,
            messages,
            message_text,
        }
    }

    pub fn sync_to_model(&mut self, journal: &Journal) {
        let items = journal
            .messages
            .iter()
            .map(|i| {
                let definition = i.get_definition();
                DecoratorBuilder::new(BorderBuilder::new(
                    WidgetBuilder::new().with_child(
                        TextBuilder::new(WidgetBuilder::new())
                            .with_text(&definition.title)
                            .build(&mut self.ui.build_ctx()),
                    ),
                ))
                .build(&mut self.ui.build_ctx())
            })
            .collect::<Vec<_>>();
        self.ui.send_message(ListViewMessage::items(
            self.messages,
            MessageDirection::ToWidget,
            items,
        ));
    }

    pub fn process_os_event(&mut self, os_event: &OsEvent, control_scheme: &ControlScheme) {
        self.ui.process_os_event(os_event);

        if let OsEvent::KeyboardInput { button, state } = *os_event {
            if state == ButtonState::Pressed {
                if let ControlButton::Key(key) = control_scheme.cursor_up.button {
                    if fyrox::utils::translate_key(key) == button {
                        self.current_message = match self.current_message {
                            None => Some(0),
                            Some(n) => Some(n.saturating_sub(1)),
                        };
                        self.ui.send_message(ListViewMessage::selection(
                            self.messages,
                            MessageDirection::ToWidget,
                            self.current_message,
                        ));
                    }
                }
                if let ControlButton::Key(key) = control_scheme.cursor_down.button {
                    if fyrox::utils::translate_key(key) == button {
                        self.current_message = match self.current_message {
                            None => Some(0),
                            Some(n) => Some(n + 1),
                        };
                        self.ui.send_message(ListViewMessage::selection(
                            self.messages,
                            MessageDirection::ToWidget,
                            self.current_message,
                        ));
                    }
                }
            }
        }
    }

    pub fn update(&mut self, delta: f32, journal: &Journal) {
        self.ui
            .update(Vector2::new(Self::WIDTH, Self::HEIGHT), delta);

        while let Some(message) = self.ui.poll_message() {
            if let Some(ListViewMessage::SelectionChanged(Some(value))) = message.data() {
                if message.direction() == MessageDirection::FromWidget {
                    if let Some(entry) = journal.messages.get(*value) {
                        self.ui.send_message(TextMessage::text(
                            self.message_text,
                            MessageDirection::ToWidget,
                            entry.get_definition().text.clone(),
                        ));
                    }
                }
            }
        }
    }
}
