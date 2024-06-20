use crate::control_scheme::ControlScheme;
use fyrox::{core::log::Log, core::visitor::prelude::*, renderer::QualitySettings};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::{
    fs::File,
    ops::{Deref, DerefMut},
};

#[derive(Debug, Clone)]
pub struct Config {
    need_save: bool,
    data: ConfigData,
}

impl Config {
    pub fn load() -> Self {
        Self {
            need_save: false,
            data: ConfigData::load(),
        }
    }

    pub fn save_if_needed(&self) {
        if self.need_save {
            self.data.save();
        }
    }
}

impl Deref for Config {
    type Target = ConfigData;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl DerefMut for Config {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.need_save = true;
        &mut self.data
    }
}

#[derive(Deserialize, Serialize, Clone, Visit, Debug)]
pub struct SoundConfig {
    pub master_volume: f32,
    pub music_volume: f32,
    pub use_hrtf: bool,
}

impl Default for SoundConfig {
    fn default() -> Self {
        Self {
            master_volume: 1.0,
            music_volume: 0.5,
            use_hrtf: true,
        }
    }
}

#[derive(Deserialize, Serialize, Debug, Default, Clone)]
pub struct ConfigData {
    pub graphics: QualitySettings,
    pub controls: ControlScheme,
    pub sound: SoundConfig,
    pub show_debug_info: bool,
}

impl ConfigData {
    const PATH: &'static str = "data/configs/settings.ron";

    fn load() -> Self {
        File::open(Self::PATH)
            .ok()
            .and_then(|file| ron::de::from_reader(file).ok())
            .unwrap_or_default()
    }

    fn save(&self) {
        let Ok(file) = File::create(Self::PATH) else {
            Log::err("Unable to save config!");
            return;
        };

        Log::verify(ron::ser::to_writer_pretty(
            file,
            self,
            PrettyConfig::default(),
        ));
    }
}
