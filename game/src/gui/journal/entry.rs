use fyrox::gui::message::MessageData;
use fyrox::{
    core::{pool::Handle, reflect::prelude::*, variable::InheritableVariable, visitor::prelude::*},
    graph::SceneGraph,
    gui::{
        define_widget_traits,
        message::UiMessage,
        text::{Text, TextMessage},
        widget::Widget,
        Control, UserInterface,
    },
};

#[derive(PartialEq, Debug, Clone)]
pub enum JournalEntryMessage {
    Text(String),
}
impl MessageData for JournalEntryMessage {}

#[derive(Visit, PartialEq, Reflect, Default, Debug, Clone)]
#[reflect(type_uuid = "ef6bad37-f3bf-4848-aff9-2c390969a308")]
#[visit(optional)]
pub struct JournalEntry {
    widget: Widget,
    title: InheritableVariable<Handle<Text>>,
}

define_widget_traits!(JournalEntry, "Journal Entry", "Journal");

impl Control for JournalEntry {
    fn handle_routed_message(&mut self, ui: &mut UserInterface, message: &mut UiMessage) {
        self.widget.handle_routed_message(ui, message);
        if let Some(JournalEntryMessage::Text(text)) = message.data_for(self.handle()) {
            ui.send(*self.title, TextMessage::Text(text.clone()));
        }
    }
}
