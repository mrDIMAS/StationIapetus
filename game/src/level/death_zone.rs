use crate::{
    character::{CharacterMessage, CharacterMessageData, DamageDealer},
    Level,
};
use fyrox::{
    core::{reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*},
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "9c258713-e44e-4366-a236-f91e09c6f0aa")]
pub struct DeathZone;

impl ScriptTrait for DeathZone {
    fn on_update(&mut self, context: &mut ScriptContext) {
        let self_bounds = context.scene.graph[context.handle].world_bounding_box();
        for &actor in Level::try_get(context.plugins).unwrap().actors.iter() {
            let character_position = context.scene.graph[actor].global_position();
            if self_bounds.is_contains_point(character_position) {
                context.message_sender.send_global(CharacterMessage {
                    character: actor,
                    data: CharacterMessageData::Damage {
                        dealer: DamageDealer {
                            entity: Default::default(),
                        },
                        hitbox: None,
                        amount: 99999.0,
                        critical_hit_probability: 0.0,
                        position: None,
                        is_melee: false,
                    },
                })
            }
        }
    }
}
