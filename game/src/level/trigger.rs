use crate::{character::character_ref, message::Message, Game, Level};
use fyrox::{
    core::{
        math::aabb::AxisAlignedBoundingBox,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    script::{ScriptContext, ScriptTrait},
};
use std::path::PathBuf;
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(Debug, Clone, Default, Visit, Reflect, AsRefStr, EnumString, EnumVariantNames)]
pub enum TriggerAction {
    #[default]
    None,
    LoadLevel {
        path: PathBuf,
    },
    EndGame,
}

#[derive(Visit, Reflect, Debug, Default, Clone)]
pub struct Trigger {
    kind: TriggerAction,
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

        if let Some(level) = Level::try_get(context.plugins) {
            let this_bounds = AxisAlignedBoundingBox::unit()
                .transform(&context.scene.graph[context.handle].global_transform());

            let player_position =
                character_ref(level.player, &context.scene.graph).position(&context.scene.graph);

            if this_bounds.is_contains_point(player_position) {
                match self.kind {
                    TriggerAction::LoadLevel { ref path } => game
                        .message_sender
                        .send(Message::LoadLevel { path: path.clone() }),
                    TriggerAction::EndGame => game.message_sender.send(Message::EndGame),
                    TriggerAction::None => {}
                }
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
