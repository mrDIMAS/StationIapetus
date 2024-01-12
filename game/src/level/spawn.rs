use crate::character::{CharacterMessage, CharacterMessageData};
use fyrox::{
    core::{
        log::Log, reflect::prelude::*, stub_uuid_provider, type_traits::prelude::*,
        visitor::prelude::*,
    },
    resource::model::{ModelResource, ModelResourceExtension},
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct DefaultWeapon(Option<ModelResource>);

stub_uuid_provider!(DefaultWeapon);

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "39c47baa-9fc3-4204-92ca-878d621f3656")]
#[visit(optional)]
pub struct CharacterSpawnPoint {
    default_weapons: Vec<DefaultWeapon>,
    prefab: Option<ModelResource>,
}

impl ScriptTrait for CharacterSpawnPoint {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        if let Some(model) = self.prefab.as_ref() {
            // Take rotation and position for the point.
            let (rotation, position) = ctx
                .scene
                .graph
                .global_rotation_position_no_scale(ctx.handle);

            let character_root_node_handle = model.instantiate(ctx.scene);

            let character_node = &mut ctx.scene.graph[character_root_node_handle];

            // Rotate the character accordingly.
            character_node
                .local_transform_mut()
                .set_position(position)
                .set_rotation(rotation);

            // Give some default weapons.
            for weapon in self.default_weapons.iter() {
                if let Some(model) = weapon.0.clone() {
                    ctx.message_sender.send_to_target(
                        character_root_node_handle,
                        CharacterMessage {
                            character: character_root_node_handle,
                            data: CharacterMessageData::AddWeapon(model),
                        },
                    )
                }
            }
        } else {
            Log::warn("Prefab is not set, nothing to spawn!")
        }
    }
}
