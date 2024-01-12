use crate::{character::character_ref, message::Message, Game};
use fyrox::{
    core::{
        math::aabb::AxisAlignedBoundingBox, reflect::prelude::*, stub_uuid_provider,
        type_traits::prelude::*, visitor::prelude::*,
    },
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

stub_uuid_provider!(TriggerAction);

#[derive(Visit, Reflect, Debug, Default, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "a7e0d266-3f3f-4100-85c5-59811f9bbab3")]
#[visit(optional)]
pub struct Trigger {
    kind: TriggerAction,
}

impl ScriptTrait for Trigger {
    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = ctx.plugins.get::<Game>();

        if let Some(level) = ctx.plugins.get::<Game>().level.as_ref() {
            let this_bounds = AxisAlignedBoundingBox::unit()
                .transform(&ctx.scene.graph[ctx.handle].global_transform());

            let player_position =
                character_ref(level.player, &ctx.scene.graph).position(&ctx.scene.graph);

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
}
