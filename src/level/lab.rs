use crate::{
    config::SoundConfig, level::BaseLevel, message::Message, player::PlayerPersistentData,
};
use rg3d::{
    core::{
        color::Color,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    resource::texture::Texture,
    scene::Scene,
};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Sender,
};

/// TODO - Implement and add plot.
/// Second level - player enters laboratory.

#[derive(Default, Visit)]
pub struct LabLevel {
    level: BaseLevel,
}

impl Deref for LabLevel {
    type Target = BaseLevel;

    fn deref(&self) -> &Self::Target {
        &self.level
    }
}

impl DerefMut for LabLevel {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.level
    }
}

impl LabLevel {
    pub async fn new(
        resource_manager: ResourceManager,
        sender: Sender<Message>,
        display_texture: Texture,
        inventory_texture: Texture,
        item_texture: Texture,
        journal_texture: Texture,
        sound_config: SoundConfig,
        persistent_data: Option<PlayerPersistentData>,
    ) -> (Self, Scene) {
        let (base_level, mut scene) = BaseLevel::new(
            "data/levels/lab.rgs",
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

        scene.ambient_lighting_color = Color::opaque(30, 30, 30);

        (Self { level: base_level }, scene)
    }
}
