use crate::character::DamageDealer;
use crate::{
    character::{CharacterMessage, CharacterMessageData},
    current_level_ref,
};
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

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct DeathZone;

impl_component_provider!(DeathZone);

impl TypeUuidProvider for DeathZone {
    fn type_uuid() -> Uuid {
        uuid!("9c258713-e44e-4366-a236-f91e09c6f0aa")
    }
}

impl ScriptTrait for DeathZone {
    fn on_update(&mut self, context: &mut ScriptContext) {
        let self_bounds = context.scene.graph[context.handle].world_bounding_box();
        for &actor in current_level_ref(context.plugins).unwrap().actors.iter() {
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
                    },
                })
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
