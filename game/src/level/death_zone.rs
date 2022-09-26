use crate::{
    character::{try_get_character_mut, CharacterCommand},
    current_level_ref,
};
use fyrox::{
    core::{
        inspect::prelude::*,
        reflect::Reflect,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    impl_component_provider,
    scene::node::TypeUuidProvider,
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Inspect, Default, Debug, Clone)]
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
            if let Some(character) = try_get_character_mut(actor, &mut context.scene.graph) {
                if self_bounds.is_contains_point(character_position) {
                    character.push_command(CharacterCommand::Damage {
                        who: Default::default(),
                        hitbox: None,
                        amount: 99999.0,
                        critical_shot_probability: 0.0,
                    });
                }
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
