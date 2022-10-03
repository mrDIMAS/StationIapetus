use crate::{
    character::{Character, CharacterCommand},
    weapon::definition::WeaponKind,
};
use fyrox::{
    core::{
        inspect::prelude::*,
        reflect::Reflect,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    resource::model::Model,
    scene::node::TypeUuidProvider,
    script::{ScriptContext, ScriptTrait},
    utils::log::Log,
};

#[derive(Visit, Reflect, Inspect, Default, Debug, Clone)]
pub struct DefaultWeapon(WeaponKind);

#[derive(Visit, Reflect, Inspect, Default, Debug, Clone)]
pub struct CharacterSpawnPoint {
    default_weapons: Vec<DefaultWeapon>,
    prefab: Option<Model>,
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

            let root = model.instantiate_geometry(ctx.scene);

            let character_node = &mut ctx.scene.graph[root];

            // Rotate the character accordingly.
            character_node
                .local_transform_mut()
                .set_position(position)
                .set_rotation(rotation);

            // Give some default weapons.
            if let Some(character) = character_node
                .script_mut()
                .and_then(|s| s.query_component_mut::<Character>())
            {
                for weapon in self.default_weapons.iter() {
                    character.push_command(CharacterCommand::AddWeapon(weapon.0))
                }
            } else {
                Log::err("Unable to find character in a prefab!")
            }
        } else {
            Log::warn("Prefab is not set, nothing to spawn!")
        }
    }

    fn restore_resources(&mut self, resource_manager: ResourceManager) {
        resource_manager
            .state()
            .containers_mut()
            .models
            .try_restore_optional_resource(&mut self.prefab);
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
