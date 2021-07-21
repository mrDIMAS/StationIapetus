use crate::message::Message;
use crate::weapon::{BaseWeapon, WeaponKind};
use rg3d::core::visitor::prelude::*;
use rg3d::engine::resource_manager::ResourceManager;
use rg3d::scene::Scene;
use std::ops::{Deref, DerefMut};
use std::sync::mpsc::Sender;

#[derive(Visit, Default)]
pub struct M4 {
    base_weapon: BaseWeapon,
}

impl Deref for M4 {
    type Target = BaseWeapon;

    fn deref(&self) -> &Self::Target {
        &self.base_weapon
    }
}

impl DerefMut for M4 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base_weapon
    }
}

impl M4 {
    pub async fn new(
        resource_manager: ResourceManager,
        scene: &mut Scene,
        sender: Sender<Message>,
    ) -> Self {
        Self {
            base_weapon: BaseWeapon::new(WeaponKind::M4, resource_manager, scene, sender).await,
        }
    }
}
