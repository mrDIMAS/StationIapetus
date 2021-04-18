use crate::player::PlayerPersistentData;
/// First level. Player just arrived to the station and start seeing weird things.
use crate::{
    config::SoundConfig,
    level::{BaseLevel, Level},
    message::Message,
};
use rg3d::core::color::Color;
use rg3d::{
    core::visitor::{Visit, VisitResult, Visitor},
    engine::resource_manager::ResourceManager,
    resource::texture::Texture,
    scene::Scene,
};
use std::{
    ops::{Deref, DerefMut},
    sync::mpsc::Sender,
};

#[derive(Default)]
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

impl Visit for ArrivalLevel {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.level.visit("Level", visitor)?;

        visitor.leave_region()
    }
}

impl ArrivalLevel {
    pub async fn new(
        resource_manager: ResourceManager,
        sender: Sender<Message>,
        display_texture: Texture,
        inventory_texture: Texture,
        item_texture: Texture,
        journal_texture: Texture,
        sound_config: SoundConfig,
        persistent_data: Option<PlayerPersistentData>,
    ) -> (Level, Scene) {
        let (base_level, mut scene) = BaseLevel::new(
            "data/levels/arrival.rgs",
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

        scene.ambient_lighting_color = Color::opaque(35, 35, 35);

        (Level::Arrival(Self { level: base_level }), scene)
    }
}
