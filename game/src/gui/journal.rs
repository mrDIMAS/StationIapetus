#![allow(dead_code)] // TODO

use crate::{
    control_scheme::{ControlButton, ControlScheme},
    gui,
};
use fyrox::asset::{impl_simple_resource, Resource};
use fyrox::core::{some_or_continue, some_or_return};
use fyrox::{
    core::{
        algebra::Vector2, pool::Handle, pool::HandlesVecExtension, reflect::prelude::*,
        visitor::prelude::*,
    },
    gui::{
        border::BorderBuilder,
        decorator::DecoratorBuilder,
        formatted_text::WrapMode,
        grid::{Column, GridBuilder, Row},
        list_view::ListView,
        list_view::{ListViewBuilder, ListViewMessage},
        message::{ButtonState, MessageDirection, OsEvent},
        scroll_viewer::ScrollViewerBuilder,
        text::Text,
        text::{TextBuilder, TextMessage},
        widget::WidgetBuilder,
        UserInterface,
    },
    resource::texture::TextureResource,
};
use std::collections::HashMap;

pub type JournalEntryId = String;

#[derive(Reflect, Visit, Debug, Clone, PartialEq, Default)]
#[reflect(type_uuid = "94fee590-fa6f-4dd2-817e-4b69490e0dfe")]
pub struct JournalEntryDefinition {
    pub title: String,
    pub text: String,
}

#[derive(Reflect, Visit, Debug, Clone, PartialEq, Default)]
#[reflect(type_uuid = "c583f54c-4458-421b-968f-4f1a7b6ca50c")]
pub struct JournalEntryDefinitionContainer {
    entries: HashMap<JournalEntryId, JournalEntryDefinition>,
}

impl_simple_resource!(
    JournalEntryDefinitionContainer,
    JournalEntryDefinitionContainerLoader,
    "journal"
);

#[derive(Default, PartialEq, Visit, Clone, Debug, Reflect)]
#[reflect(type_uuid = "fcc8459a-5944-42e4-a22f-24babd497053")]
pub struct Journal {
    messages: Vec<JournalEntryId>,
    resource: Option<Resource<JournalEntryDefinitionContainer>>,
}

#[derive(Visit, PartialEq, Debug)]
pub struct JournalDisplay {
    pub ui: UserInterface,
    pub render_target: TextureResource,
    objective: Handle<Text>,
    messages: Handle<ListView>,
    message_text: Handle<Text>,
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

        let render_target = gui::create_ui_render_target(Self::WIDTH, Self::HEIGHT);

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
        dbg!(journal.resource.is_some());
        let resource = some_or_return!(journal.resource.as_ref()).data_ref();
        dbg!();
        let resource = some_or_return!(resource.as_loaded_ref());
        dbg!();
        let items = journal
            .messages
            .iter()
            .filter_map(|i| {
                let definition = resource.entries.get(i)?;
                Some(
                    DecoratorBuilder::new(BorderBuilder::new(
                        WidgetBuilder::new().with_child(
                            TextBuilder::new(WidgetBuilder::new())
                                .with_text(&definition.title)
                                .build(&mut self.ui.build_ctx()),
                        ),
                    ))
                    .build(&mut self.ui.build_ctx()),
                )
            })
            .collect::<Vec<_>>();
        dbg!(items.len());
        self.ui
            .send(self.messages, ListViewMessage::Items(items.to_base()));
    }

    pub fn process_os_event(&mut self, os_event: &OsEvent, control_scheme: &ControlScheme) {
        self.ui.process_os_event(os_event);

        if let OsEvent::KeyboardInput { button, state, .. } = *os_event {
            if state == ButtonState::Pressed {
                if let ControlButton::Key(key) = control_scheme.cursor_up.button {
                    if fyrox::utils::translate_key_to_ui(*key) == button {
                        self.current_message = match self.current_message {
                            None => Some(0),
                            Some(n) => Some(n.saturating_sub(1)),
                        };
                        self.ui.send(
                            self.messages,
                            ListViewMessage::Selection(
                                self.current_message.map(|n| vec![n]).unwrap_or_default(),
                            ),
                        );
                    }
                }
                if let ControlButton::Key(key) = control_scheme.cursor_down.button {
                    if fyrox::utils::translate_key_to_ui(*key) == button {
                        self.current_message = match self.current_message {
                            None => Some(0),
                            Some(n) => Some(n + 1),
                        };
                        self.ui.send(
                            self.messages,
                            ListViewMessage::Selection(
                                self.current_message.map(|n| vec![n]).unwrap_or_default(),
                            ),
                        );
                    }
                }
            }
        }
    }

    pub fn update(&mut self, delta: f32, journal: &Journal) {
        self.ui.update(
            Vector2::new(Self::WIDTH, Self::HEIGHT),
            delta,
            &Default::default(),
        );

        while let Some(message) = self.ui.poll_message() {
            if let Some(ListViewMessage::Selection(value)) = message.data() {
                if message.direction() == MessageDirection::FromWidget {
                    if let Some(entry) =
                        value.first().cloned().and_then(|n| journal.messages.get(n))
                    {
                        let resource = some_or_continue!(journal.resource.as_ref()).data_ref();
                        let resource = some_or_continue!(resource.as_loaded_ref());
                        if let Some(definition) = resource.entries.get(entry) {
                            self.ui.send(
                                self.message_text,
                                TextMessage::Text(definition.text.clone()),
                            );
                        }
                    }
                }
            }
        }
    }
}
