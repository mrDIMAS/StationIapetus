use crate::{config::SoundConfig, level::BaseLevel, player::PlayerPersistentData, MessageSender};
use rg3d::{
    core::{color::Color, visitor::prelude::*},
    engine::resource_manager::ResourceManager,
    resource::texture::Texture,
    scene::Scene,
};
use std::ops::{Deref, DerefMut};

#[derive(Default, Visit)]
pub struct TestbedLevel {
    level: BaseLevel,
}

impl Deref for TestbedLevel {
    type Target = BaseLevel;

    fn deref(&self) -> &Self::Target {
        &self.level
    }
}

impl DerefMut for TestbedLevel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.level
    }
}

impl TestbedLevel {
    pub async fn new(
        resource_manager: ResourceManager,
        sender: MessageSender,
        display_texture: Texture,
        inventory_texture: Texture,
        item_texture: Texture,
        journal_texture: Texture,
        sound_config: SoundConfig,
        persistent_data: Option<PlayerPersistentData>,
    ) -> (Self, Scene) {
        let (base_level, mut scene) = BaseLevel::new(
            "data/levels/testbed.rgs",
            resource_manager,
            sender,
            display_texture,
            inventory_texture,
            item_texture,
            journal_texture,
            sound_config,
            persistent_data,
        )
        .await;

        scene.ambient_lighting_color = Color::opaque(100, 100, 100);

        (Self { level: base_level }, scene)
    }
}
