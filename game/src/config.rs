use crate::control_scheme::ControlScheme;
use fyrox::core::futures::executor::block_on;
use fyrox::{core::log::Log, core::visitor::prelude::*, renderer::QualitySettings};
use std::ops::{Deref, DerefMut};

#[derive(Debug, PartialEq, Clone)]
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

    pub fn save_if_needed(&mut self) {
        if self.need_save {
            self.data.save();
            self.need_save = false;
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

#[derive(PartialEq, Clone, Visit, Debug)]
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

#[derive(Visit, PartialEq, Debug, Default, Clone)]
pub struct ConfigData {
    pub graphics: QualitySettings,
    pub controls: ControlScheme,
    pub sound: SoundConfig,
    pub show_debug_info: bool,
}

impl ConfigData {
    const PATH: &'static str = "data/configs/settings.ron";

    fn load() -> Self {
        block_on(Visitor::load_ascii_from_file(Self::PATH))
            .map(|mut v| {
                let mut data = ConfigData::default();
                if data.visit("Data", &mut v).is_err() {
                    data = ConfigData::default();
                }
                data
            })
            .unwrap_or_default()
    }

    fn save(&mut self) {
        let mut visitor = Visitor::new();
        Log::verify(self.visit("Data", &mut visitor));
        Log::verify(visitor.save_ascii_to_file(Self::PATH));
    }
}
