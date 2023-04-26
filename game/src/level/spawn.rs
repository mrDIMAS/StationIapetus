use crate::{
    character::{CharacterMessage, CharacterMessageData},
    weapon::definition::WeaponKind,
};
use fyrox::{
    core::{
        log::Log,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    resource::model::{ModelResource, ModelResourceExtension},
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct DefaultWeapon(WeaponKind);

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct CharacterSpawnPoint {
    default_weapons: Vec<DefaultWeapon>,
    prefab: Option<ModelResource>,
}

impl_component_provider!(CharacterSpawnPoint);

impl TypeUuidProvider for CharacterSpawnPoint {
    fn type_uuid() -> Uuid {
        uuid!("39c47baa-9fc3-4204-92ca-878d621f3656")
    }
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
                ctx.message_sender.send_to_target(
                    character_root_node_handle,
                    CharacterMessage {
                        character: character_root_node_handle,
                        data: CharacterMessageData::AddWeapon(weapon.0),
                    },
                )
            }
        } else {
            Log::warn("Prefab is not set, nothing to spawn!")
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
