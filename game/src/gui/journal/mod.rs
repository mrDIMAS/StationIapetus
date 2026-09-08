pub mod entry;

use crate::{
    control_scheme::{ControlButton, ControlScheme},
    gui::journal::entry::JournalEntryMessage,
};
use fyrox::core::algebra::Vector2;
use fyrox::gui::texture::TexturePixelKind;
use fyrox::resource::texture::TextureResourceExtension;
use fyrox::{
    asset::{impl_simple_resource, Resource},
    core::{
        pool::Handle, pool::HandlesVecExtension, reflect::prelude::*, some_or_return,
        visitor::prelude::*,
    },
    gui::{
        list_view::{ListView, ListViewMessage},
        message::{ButtonState, MessageDirection, OsEvent, UiMessage},
        text::{Text, TextMessage},
        texture::TextureResource,
        UiContainer, UserInterface, UserInterfaceResourceExtension,
    },
    plugin::{error::GameResult, PluginContext},
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

#[derive(Visit, PartialEq, Reflect, Default, Debug, Clone)]
#[reflect(type_uuid = "9d94ec70-fb24-4c37-b5e5-0e36b6defa61")]
#[visit(optional)]
pub struct JournalDisplayData {
    objective: Handle<Text>,
    messages: Handle<ListView>,
    message_text: Handle<Text>,
    entry_prefab: Option<Resource<UserInterface>>,
}

#[derive(Visit, Default, PartialEq, Debug)]
pub struct JournalDisplay {
    pub ui: Handle<UserInterface>,
    pub data: JournalDisplayData,
    current_message: Option<usize>,
}

impl JournalDisplay {
    const WIDTH: u32 = 512;
    const HEIGHT: u32 = 400;

    pub fn new(mut ui: UserInterface, ctx: &mut PluginContext) -> Self {
        let data = ui.user_data.try_take_or_default::<JournalDisplayData>();
        ui.render_target = Some(TextureResource::new_render_target_with_format(
            Self::WIDTH,
            Self::HEIGHT,
            TexturePixelKind::SRGB8,
        ));
        ui.set_screen_size(Vector2::new(Self::WIDTH as f32, Self::HEIGHT as f32));
        Self {
            ui: ctx.user_interfaces.add(ui),
            data,
            current_message: None,
        }
    }

    pub fn render_target(&self, ui_container: &UiContainer) -> TextureResource {
        ui_container
            .try_get(self.ui)
            .ok()
            .and_then(|ui| ui.render_target.clone())
            .unwrap_or_default()
    }

    pub fn sync_to_model(&mut self, uis: &mut UiContainer, journal: &Journal) -> GameResult {
        let ui = uis.try_get_mut(self.ui)?;
        let resource = some_or_return!(journal.resource.as_ref(), Ok(())).data_ref();
        let resource = some_or_return!(resource.as_loaded_ref(), Ok(()));
        if let Some(data) = self.data.entry_prefab.as_ref() {
            let items = journal
                .messages
                .iter()
                .filter_map(|i| {
                    let definition = resource.entries.get(i)?;
                    let (instance, _) = data.instantiate(ui);
                    ui.send(instance, JournalEntryMessage::Text(definition.text.clone()));
                    Some(instance)
                })
                .collect::<Vec<_>>();
            ui.send(self.data.messages, ListViewMessage::Items(items.to_base()));
        }
        Ok(())
    }

    fn move_selection(&mut self, ui: &UserInterface, dir: isize) {
        self.current_message = match self.current_message {
            None => Some(0),
            Some(n) => Some(n.saturating_add_signed(dir)),
        };
        ui.send(
            self.data.messages,
            ListViewMessage::Selection(self.current_message.map(|n| vec![n]).unwrap_or_default()),
        );
    }

    pub fn process_os_event(
        &mut self,
        uis: &mut UiContainer,
        os_event: &OsEvent,
        control_scheme: &ControlScheme,
    ) -> GameResult {
        let ui = uis.try_get_mut(self.ui)?;
        if let OsEvent::KeyboardInput { button, state, .. } = *os_event {
            if state == ButtonState::Pressed {
                if let ControlButton::Key(key) = control_scheme.cursor_up.button {
                    if fyrox::utils::translate_key_to_ui(*key) == button {
                        self.move_selection(ui, -1);
                    }
                }
                if let ControlButton::Key(key) = control_scheme.cursor_down.button {
                    if fyrox::utils::translate_key_to_ui(*key) == button {
                        self.move_selection(ui, 1);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn handle_ui_message(
        &mut self,
        uis: &mut UiContainer,
        ui_handle: Handle<UserInterface>,
        message: &UiMessage,
        journal: &Journal,
    ) -> GameResult {
        if self.ui != ui_handle {
            return Ok(());
        }
        let ui = uis.try_get_mut(self.ui)?;
        if let Some(ListViewMessage::Selection(value)) = message.data() {
            if message.direction() == MessageDirection::FromWidget {
                if let Some(entry) = value.first().cloned().and_then(|n| journal.messages.get(n)) {
                    let resource = some_or_return!(journal.resource.as_ref(), Ok(())).data_ref();
                    let resource = some_or_return!(resource.as_loaded_ref(), Ok(()));
                    if let Some(definition) = resource.entries.get(entry) {
                        ui.send(
                            self.data.message_text,
                            TextMessage::Text(definition.text.clone()),
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
