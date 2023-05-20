use crate::{character::character_ref, message::Message, Game, Level};
use fyrox::{
    core::{
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    script::{ScriptContext, ScriptTrait},
};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(Debug, Clone, Visit, Reflect, AsRefStr, EnumString, EnumVariantNames)]
pub enum TriggerKind {
    NextLevel,
    EndGame,
}

impl Default for TriggerKind {
    fn default() -> Self {
        Self::NextLevel
    }
}

#[derive(Visit, Reflect, Debug, Default, Clone)]
pub struct Trigger {
    kind: TriggerKind,
}

impl_component_provider!(Trigger);

impl TypeUuidProvider for Trigger {
    fn type_uuid() -> Uuid {
        uuid!("a7e0d266-3f3f-4100-85c5-59811f9bbab3")
    }
}

impl ScriptTrait for Trigger {
    fn on_update(&mut self, context: &mut ScriptContext) {
        let game = Game::game_ref(context.plugins);

        let position = context.scene.graph[context.handle].global_position();

        if let Some(level) = Level::try_get(context.plugins) {
            for actor in level.actors.iter() {
                let actor_position =
                    character_ref(*actor, &context.scene.graph).position(&context.scene.graph);

                if actor_position.metric_distance(&position) < 1.0 {
                    match self.kind {
                        TriggerKind::NextLevel => game.message_sender.send(Message::LoadNextLevel),
                        TriggerKind::EndGame => game.message_sender.send(Message::EndGame),
                    }
                }
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
