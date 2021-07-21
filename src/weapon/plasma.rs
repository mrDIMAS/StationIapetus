use crate::message::Message;
use crate::weapon::{BaseWeapon, WeaponKind};
use rg3d::core::visitor::prelude::*;
use rg3d::engine::resource_manager::ResourceManager;
use rg3d::scene::Scene;
use std::ops::{Deref, DerefMut};
use std::sync::mpsc::Sender;

#[derive(Visit, Default)]
pub struct PlasmaGun {
    base_weapon: BaseWeapon,
}

impl Deref for PlasmaGun {
    type Target = BaseWeapon;

    fn deref(&self) -> &Self::Target {
        &self.base_weapon
    }
}

impl DerefMut for PlasmaGun {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.base_weapon
    }
}

impl PlasmaGun {
    pub async fn new(
        resource_manager: ResourceManager,
        scene: &mut Scene,
        sender: Sender<Message>,
    ) -> Self {
        Self {
            base_weapon: BaseWeapon::new(WeaponKind::PlasmaRifle, resource_manager, scene, sender)
                .await,
        }
    }
}
