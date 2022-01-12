use crate::{config::SoundConfig, level::BaseLevel, player::PlayerPersistentData, MessageSender};
use fyrox::{
    core::{
        color::Color,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    resource::texture::Texture,
    scene::Scene,
};
use std::ops::{Deref, DerefMut};

/// First level. Player just arrived to the station and start seeing weird things.

#[derive(Default, Visit)]
pub struct ArrivalLevel {
    level: BaseLevel,
}

impl Deref for ArrivalLevel {
    type Target = BaseLevel;

    fn deref(&self) -> &Self::Target {
        &self.level
    }
}

impl DerefMut for ArrivalLevel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.level
    }
}

impl ArrivalLevel {
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
            "data/levels/loading_bay.rgs",
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

        scene.ambient_lighting_color = Color::opaque(50, 50, 50);

        (Self { level: base_level }, scene)
    }
}
