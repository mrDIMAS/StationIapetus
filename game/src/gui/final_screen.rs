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
#[type_uuid(id = "ddbbaf2f-7519-455a-901e-eaffae3a4849")]
pub struct FinalScreenData {
    exit_to_menu: Handle<Button>,
    exit_game: Handle<Button>,
}

#[derive(Visit, Default, Debug)]
pub struct FinalScreen {
    ui: Handle<UserInterface>,
    data: FinalScreenData,
}

impl FinalScreen {
    pub fn new(mut ui: UserInterface, ctx: &mut PluginContext) -> Self {
        let data = ui
            .user_data
            .try_take::<FinalScreenData>()
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
        if let Some(ButtonMessage::Click) = message.data_from(self.data.exit_to_menu) {
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
