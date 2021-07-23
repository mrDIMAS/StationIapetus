use crate::{
    item::ItemKind,
    weapon::projectile::{Damage, ProjectileKind},
};
use rg3d::{
    core::{algebra::Vector3, rand::Rng, visitor::prelude::*},
    lazy_static::lazy_static,
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Hash, Visit)]
#[repr(u32)]
pub enum WeaponKind {
    M4 = 0,
    Ak47 = 1,
    PlasmaRifle = 2,
    Glock = 3,
    RailGun = 4,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Hash)]
#[repr(u32)]
pub enum ShotEffect {
    Smoke = 0,
    Beam = 1,
    Rail = 2,
}

impl Default for WeaponKind {
    fn default() -> Self {
        Self::M4
    }
}

impl WeaponKind {
    pub fn associated_item(&self) -> ItemKind {
        match self {
            WeaponKind::M4 => ItemKind::M4,
            WeaponKind::Ak47 => ItemKind::Ak47,
            WeaponKind::PlasmaRifle => ItemKind::PlasmaGun,
            WeaponKind::Glock => ItemKind::Glock,
            WeaponKind::RailGun => ItemKind::RailGun,
        }
    }
}

#[derive(Copy, Clone, Debug, Deserialize)]
pub enum WeaponProjectile {
    Projectile(ProjectileKind),
    /// For high-speed "projectiles".
    Ray {
        damage: Damage,
    },
}

#[derive(Deserialize)]
pub struct WeaponDefinition {
    pub model: String,
    pub shot_sounds: Vec<String>,
    pub projectile: WeaponProjectile,
    pub shoot_interval: f64,
    pub yaw_correction: f32,
    pub pitch_correction: f32,
    pub ammo_indicator_offset: (f32, f32, f32),
    pub ammo_consumption_per_shot: u32,
    pub v_recoil: (f32, f32),
    pub h_recoil: (f32, f32),
    pub shot_effect: ShotEffect,
}

impl WeaponDefinition {
    pub fn ammo_indicator_offset(&self) -> Vector3<f32> {
        Vector3::new(
            self.ammo_indicator_offset.0,
            self.ammo_indicator_offset.1,
            self.ammo_indicator_offset.2,
        )
    }

    pub fn gen_v_recoil_angle(&self) -> f32 {
        rg3d::rand::thread_rng()
            .gen_range(self.v_recoil.0.to_radians()..self.v_recoil.1.to_radians())
    }

    pub fn gen_h_recoil_angle(&self) -> f32 {
        rg3d::rand::thread_rng()
            .gen_range(self.h_recoil.0.to_radians()..self.h_recoil.1.to_radians())
    }
}

#[derive(Deserialize, Default)]
pub struct WeaponDefinitionContainer {
    pub map: HashMap<WeaponKind, WeaponDefinition>,
}

impl WeaponDefinitionContainer {
    pub fn new() -> Self {
        let file = File::open("data/configs/weapons.ron").unwrap();
        ron::de::from_reader(file).unwrap()
    }
}

lazy_static! {
    pub static ref DEFINITIONS: WeaponDefinitionContainer = WeaponDefinitionContainer::new();
}
