use crate::{message::Message, MessageSender};
use fyrox::{
    core::{pool::Handle, reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*},
    gui::{
        button::{Button, ButtonMessage},
        message::UiMessage,
        UserInterface,
    },
    plugin::PluginContext,
};

#[derive(Visit, Default, Debug, Clone, TypeUuidProvider, Reflect)]
#[type_uuid(id = "41a8bf46-132e-426e-8255-67c78b967002")]
pub struct DeathScreenData {
    load_game: Handle<Button>,
    exit_to_menu: Handle<Button>,
    exit_game: Handle<Button>,
}

#[derive(Visit, Default, Debug)]
pub struct DeathScreen {
    ui: Handle<UserInterface>,
    data: DeathScreenData,
}

impl DeathScreen {
    pub fn new(mut ui: UserInterface, ctx: &mut PluginContext) -> Self {
        let data = ui
            .user_data
            .try_take::<DeathScreenData>()
            .unwrap_or_default();

        Self {
            ui: ctx.user_interfaces.add(ui),
            data,
        }
    }

    pub fn handle_ui_message(
        self,
        ctx: &mut PluginContext,
        ui_handle: Handle<UserInterface>,
        message: &UiMessage,
        sender: &MessageSender,
    ) -> Option<Self> {
        if self.ui != ui_handle {
            return Some(self);
        }

        if let Some(ButtonMessage::Click) = message.data_from(self.data.load_game) {
            // TODO: Add quick saves.
            // sender.send(Message::LoadGame);
            self.destroy(ctx)
        } else if let Some(ButtonMessage::Click) = message.data_from(self.data.exit_to_menu) {
            sender.send(Message::ToggleMainMenu);
            self.destroy(ctx)
        } else if let Some(ButtonMessage::Click) = message.data_from(self.data.exit_game) {
            sender.send(Message::QuitGame);
            self.destroy(ctx)
        } else {
            Some(self)
        }
    }

    pub fn destroy(self, ctx: &mut PluginContext) -> Option<Self> {
        ctx.user_interfaces.remove(self.ui);
        None
    }
}
