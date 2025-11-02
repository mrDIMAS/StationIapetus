use crate::{message::Message, MessageSender};
use chrono::{DateTime, Utc};
use fyrox::{
    core::{log::Log, pool::Handle, reflect::prelude::*, visitor::prelude::*},
    gui::{
        border::BorderBuilder,
        button::{ButtonBuilder, ButtonMessage},
        decorator::DecoratorBuilder,
        font::FontResource,
        grid::{Column, GridBuilder, Row},
        list_view::{ListViewBuilder, ListViewMessage},
        message::{MessageDirection, UiMessage},
        stack_panel::StackPanelBuilder,
        text::{TextBuilder, TextMessage},
        text_box::{TextBoxBuilder, TextCommitMode},
        widget::{WidgetBuilder, WidgetMessage},
        window::{WindowBuilder, WindowMessage, WindowTitle},
        BuildContext, HorizontalAlignment, Orientation, Thickness, UiNode, UserInterface,
        VerticalAlignment,
    },
};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::SystemTime,
};

#[derive(Visit, Reflect, Clone, Default, Debug)]
pub enum Mode {
    #[default]
    Save,
    Load,
}

#[derive(Default, Debug, Visit, Clone, Reflect)]
pub struct SaveLoadDialog {
    pub window: Handle<UiNode>,
    confirm: Handle<UiNode>,
    cancel: Handle<UiNode>,
    name: Handle<UiNode>,
    saved_games: Handle<UiNode>,
    saved_games_list: Vec<PathBuf>,
    mode: Mode,
    file_stem: String,
    selected_entry: Option<usize>,
}

fn create_saved_game_entry(
    path: &Path,
    font: FontResource,
    ctx: &mut BuildContext,
) -> Handle<UiNode> {
    let text = format!(
        "{} - {}",
        path.file_stem().unwrap_or_default().to_string_lossy(),
        DateTime::<Utc>::from(
            path.metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or_else(|_| SystemTime::now())
        )
        .format("%d/%m/%Y %H:%M")
    );

    let text_handle = TextBuilder::new(WidgetBuilder::new())
        .with_vertical_text_alignment(VerticalAlignment::Center)
        .with_horizontal_text_alignment(HorizontalAlignment::Left)
        .with_font(font.clone())
        .with_font_size(16.0.into())
        .with_text(text)
        .build(ctx);

    DecoratorBuilder::new(BorderBuilder::new(
        WidgetBuilder::new().with_child(text_handle),
    ))
    .build(ctx)
}

fn is_file_stem_valid(file_stem: &str) -> bool {
    file_stem.chars().all(|c| c.is_alphanumeric() || c == '_')
}

impl SaveLoadDialog {
    const SAVED_GAMES_FOLDER: &'static str = "./saved_games";

    pub fn new(mode: Mode, font: FontResource, ctx: &mut BuildContext) -> Self {
        let file_stem = "unnamed_save";

        let mut items = Vec::new();
        let mut saved_games_list = Vec::new();
        if let Ok(dir_iterator) = std::fs::read_dir(Self::SAVED_GAMES_FOLDER) {
            for entry in dir_iterator.flatten() {
                let path = entry.path();
                if let Some(extension) = path.extension() {
                    if extension == OsStr::new("save") {
                        items.push(create_saved_game_entry(&path, font.clone(), ctx));
                        saved_games_list.push(path);
                    }
                }
            }
        }

        let name = TextBoxBuilder::new(
            WidgetBuilder::new()
                .on_row(0)
                .with_margin(Thickness::uniform(1.0))
                .with_visibility(matches!(mode, Mode::Save)),
        )
        .with_text(file_stem)
        .with_font(font.clone())
        .with_font_size(18.0.into())
        .with_text_commit_mode(TextCommitMode::Immediate)
        .build(ctx);

        let saved_games = ListViewBuilder::new(
            WidgetBuilder::new()
                .with_margin(Thickness::uniform(1.0))
                .on_row(1),
        )
        .with_items(items)
        .build(ctx);

        let (title_text, confirm_text) = match mode {
            Mode::Save => ("Save Game", "Save"),
            Mode::Load => ("Load Game", "Load"),
        };

        let confirm = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(120.0)
                .with_margin(Thickness::uniform(2.0))
                .with_enabled(matches!(mode, Mode::Save)),
        )
        .with_text_and_font_size(confirm_text, font.clone(), 24.0.into())
        .build(ctx);

        let cancel = ButtonBuilder::new(
            WidgetBuilder::new()
                .with_width(120.0)
                .with_margin(Thickness::uniform(2.0)),
        )
        .with_text_and_font_size("Cancel", font.clone(), 24.0.into())
        .build(ctx);

        let content = GridBuilder::new(
            WidgetBuilder::new()
                .with_child(name)
                .with_child(saved_games)
                .with_child(
                    StackPanelBuilder::new(
                        WidgetBuilder::new()
                            .with_horizontal_alignment(HorizontalAlignment::Right)
                            .on_row(2)
                            .with_child(confirm)
                            .with_child(cancel),
                    )
                    .with_orientation(Orientation::Horizontal)
                    .build(ctx),
                ),
        )
        .add_row(Row::auto())
        .add_row(Row::stretch())
        .add_row(Row::strict(35.0))
        .add_column(Column::stretch())
        .build(ctx);

        let window = WindowBuilder::new(WidgetBuilder::new().with_width(400.0).with_height(500.0))
            .can_minimize(false)
            .can_maximize(false)
            .open(false)
            .with_title(WindowTitle::text(title_text))
            .with_content(content)
            .build(ctx);

        ctx.inner().send(
            window,
            WindowMessage::OpenModal {
                center: true,
                focus_content: true,
            },
        );

        Self {
            window,
            confirm,
            cancel,
            name,
            saved_games,
            saved_games_list,
            mode,
            file_stem: file_stem.to_string(),
            selected_entry: None,
        }
    }

    pub fn handle_ui_message(
        mut self,
        message: &UiMessage,
        ui: &mut UserInterface,
        sender: &MessageSender,
    ) -> Option<Self> {
        if let Some(WindowMessage::Close) = message.data() {
            if message.destination() == self.window {
                self.destroy(ui);
                return None;
            }
        } else if let Some(ButtonMessage::Click) = message.data() {
            let mut close = false;
            if message.destination() == self.confirm {
                match self.mode {
                    Mode::Save => {
                        let folder = Path::new(Self::SAVED_GAMES_FOLDER);
                        let path = folder.join(self.file_stem.clone() + ".save");

                        if !folder.exists() {
                            Log::verify(std::fs::create_dir_all(folder));
                        }

                        sender.send(Message::SaveGame(path))
                    }
                    Mode::Load => {
                        if let Some(path) = self
                            .selected_entry
                            .and_then(|index| self.saved_games_list.get(index).cloned())
                        {
                            if path.exists() {
                                sender.send(Message::LoadGame(path))
                            }
                        }
                    }
                }
                close = true;
            } else if message.destination() == self.cancel {
                ui.send(self.window, WindowMessage::Close);
                close = true;
            }

            if close {
                ui.send(self.window, WindowMessage::Close);
            }
        } else if let Some(TextMessage::Text(text)) = message.data() {
            if message.destination() == self.name
                && message.direction() == MessageDirection::FromWidget
            {
                self.file_stem.clone_from(text);

                if matches!(self.mode, Mode::Save) {
                    ui.send(
                        self.confirm,
                        WidgetMessage::Enabled(is_file_stem_valid(&self.file_stem)),
                    );
                }
            }
        } else if let Some(ListViewMessage::Selection(index)) = message.data() {
            if message.destination() == self.saved_games
                && message.direction() == MessageDirection::FromWidget
            {
                self.selected_entry = index.first().cloned();

                if let Some(file_stem) = self
                    .selected_entry
                    .and_then(|index| self.saved_games_list.get(index))
                    .and_then(|path| path.file_stem())
                {
                    self.file_stem = file_stem.to_string_lossy().to_string();

                    ui.send(self.name, TextMessage::Text(self.file_stem.clone()));
                }

                ui.send(
                    self.confirm,
                    WidgetMessage::Enabled(
                        self.selected_entry.is_some() && is_file_stem_valid(&self.file_stem),
                    ),
                );
            }
        }

        Some(self)
    }

    fn destroy(self, ui: &mut UserInterface) {
        ui.send(self.window, WidgetMessage::Remove);
    }
}
