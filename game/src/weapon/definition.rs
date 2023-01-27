use crate::level::item::ItemKind;
use fyrox::{
    core::{reflect::prelude::*, visitor::prelude::*},
    lazy_static::lazy_static,
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Deserialize,
    Hash,
    Visit,
    Reflect,
    AsRefStr,
    EnumString,
    EnumVariantNames,
)]
#[repr(u32)]
pub enum WeaponKind {
    M4 = 0,
    Ak47 = 1,
    PlasmaRifle = 2,
    Glock = 3,
    RailGun = 4,
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

#[derive(Deserialize, Debug)]
pub struct WeaponDefinition {
    pub model: String,
    pub shot_sounds: Vec<String>,
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
