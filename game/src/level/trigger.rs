use crate::{
    character::{try_get_character_ref, Character},
    message::Message,
    Game,
};
use fyrox::{
    core::{
        math::aabb::AxisAlignedBoundingBox, pool::Handle, reflect::prelude::*, stub_uuid_provider,
        type_traits::prelude::*, visitor::prelude::*,
    },
    fxhash::FxHashSet,
    graph::BaseSceneGraph,
    scene::node::Node,
    script::{ScriptContext, ScriptTrait},
};
use std::path::PathBuf;
use strum_macros::{AsRefStr, EnumString, VariantNames};

#[derive(Debug, Clone, Default, Visit, Reflect)]
pub struct BotCounter {
    counter: usize,
    #[reflect(hidden)]
    actors: FxHashSet<Handle<Node>>,
    despawn: bool,
}

#[derive(Debug, Clone, Default, Visit, Reflect, AsRefStr, EnumString, VariantNames)]
pub enum TriggerAction {
    #[default]
    None,
    LoadLevel {
        path: PathBuf,
    },
    BotCounter(BotCounter),
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

        if let Some(level) = game.level.as_ref() {
            let this_bounds = AxisAlignedBoundingBox::unit()
                .transform(&ctx.scene.graph[ctx.handle].global_transform());

            let contains_player = try_get_character_ref(level.player, &ctx.scene.graph)
                .map(|c| c.position(&ctx.scene.graph))
                .is_some_and(|pos| this_bounds.is_contains_point(pos));

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

                        if let Some(actor_ref) = ctx
                            .scene
                            .graph
                            .try_get_script_component_of::<Character>(*actor)
                        {
                            let actor_position = ctx.scene.graph[actor_ref.body].global_position();

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
    }
}
