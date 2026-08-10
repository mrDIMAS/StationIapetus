use crate::bot::Bot;
use crate::player::Player;
use crate::{message::Message, Game};
use fyrox::plugin::error::GameResult;
use fyrox::{
    core::{
        math::aabb::AxisAlignedBoundingBox,
        pool::Handle,
        reflect::prelude::*,
        type_traits::{ComponentProvider, TypeUuidProvider},
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    fxhash::FxHashSet,
    graph::SceneGraph,
    scene::node::Node,
    script::{ScriptContext, ScriptTrait},
};
use std::path::PathBuf;
use strum_macros::{AsRefStr, EnumString, VariantNames};

#[derive(Debug, Clone, Default, Visit, PartialEq, Reflect, TypeUuidProvider)]
#[type_uuid(id = "c8a4985a-f670-4e96-9fc5-39db4b7ebbbb")]
pub struct BotCounter {
    counter: usize,
    #[reflect(hidden)]
    actors: FxHashSet<Handle<Node>>,
    despawn: bool,
}

#[derive(
    Debug,
    Clone,
    Default,
    Visit,
    Reflect,
    AsRefStr,
    PartialEq,
    EnumString,
    VariantNames,
    TypeUuidProvider,
)]
#[type_uuid(id = "fbc19c97-0000-4471-bda0-32623f626ef0")]
pub enum TriggerAction {
    #[default]
    None,
    LoadLevel {
        path: PathBuf,
    },
    BotCounter(BotCounter),
    EndGame,
}

#[derive(Visit, PartialEq, Reflect, Debug, Default, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "a7e0d266-3f3f-4100-85c5-59811f9bbab3")]
#[visit(optional)]
pub struct Trigger {
    kind: TriggerAction,
}

impl ScriptTrait for Trigger {
    fn on_update(&mut self, ctx: &mut ScriptContext) -> GameResult {
        let game = ctx.plugins.get::<Game>();

        if let Some(level) = game.level.as_ref() {
            let this_bounds = AxisAlignedBoundingBox::unit()
                .transform(&ctx.scene.graph[ctx.handle].global_transform());

            // Cek posisi player langsung via script component Player
            let contains_player = ctx
                .scene
                .graph
                .try_get_script_component_of::<Player>(level.player)
                .map(|player| this_bounds.is_contains_point(player.position))
                .unwrap_or(false);

            match self.kind {
                TriggerAction::LoadLevel { ref path } => {
                    if contains_player {
                        game.message_sender
                            .send(Message::LoadLevel { path: path.clone() })
                    }
                }
                TriggerAction::EndGame => {
                    if contains_player {
                        game.message_sender.send(Message::EndGame)
                    }
                }
                TriggerAction::None => {}
                TriggerAction::BotCounter(ref mut bot_counter) => {
                    let mut despawn_list = Vec::new();

                    for actor in level.actors.iter() {
                        if *actor == level.player {
                            continue;
                        }

                        // Dapatkan referensi body dari Player atau Bot
                        let body_handle = if let Ok(player) = ctx
                            .scene
                            .graph
                            .try_get_script_component_of::<Player>(*actor)
                        {
                            Some(player.body)
                        } else if let Ok(bot) =
                            ctx.scene.graph.try_get_script_component_of::<Bot>(*actor)
                        {
                            Some(bot.body)
                        } else {
                            None
                        };

                        if let Some(body) = body_handle {
                            let actor_position = ctx.scene.graph[body].global_position();

                            if this_bounds.is_contains_point(actor_position)
                                && !bot_counter.actors.contains(actor)
                            {
                                bot_counter.counter += 1;
                                bot_counter.actors.insert(*actor);

                                if bot_counter.despawn {
                                    despawn_list.push(*actor);
                                }
                            }
                        }
                    }

                    for handle in despawn_list {
                        ctx.scene.graph.remove_node(handle);
                    }
                }
            }
        }
        Ok(())
    }
}
