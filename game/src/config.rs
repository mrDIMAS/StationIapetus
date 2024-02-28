use crate::control_scheme::ControlScheme;
use fyrox::engine::GraphicsContext;
use fyrox::{plugin::PluginContext, renderer::QualitySettings};
use ron::ser::PrettyConfig;
use serde::{Deserialize, Serialize};
use std::fs::File;

#[derive(Deserialize, Serialize, Clone)]
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

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Config {
    pub graphics_settings: QualitySettings,
    pub controls: ControlScheme,
    pub sound: SoundConfig,
    pub show_debug_info: bool,
}

#[derive(Debug)]
pub enum ConfigError {
    Io(std::io::Error),
    RonSpanned(ron::error::SpannedError),
    Ron(ron::Error),
    Custom(String),
}

impl From<std::io::Error> for ConfigError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}

impl From<ron::error::SpannedError> for ConfigError {
    fn from(e: ron::error::SpannedError) -> Self {
        Self::RonSpanned(e)
    }
}

impl From<ron::Error> for ConfigError {
    fn from(e: ron::Error) -> Self {
        Self::Ron(e)
    }
}

impl Config {
    const PATH: &'static str = "data/configs/settings.ron";

    pub fn load() -> Result<Self, ConfigError> {
        let file = File::open(Self::PATH)?;
        Ok(ron::de::from_reader(file)?)
    }

    pub fn save(
        context: &PluginContext,
        control_scheme: ControlScheme,
        sound_config: SoundConfig,
        show_debug_info: bool,
    ) -> Result<(), ConfigError> {
        if let GraphicsContext::Initialized(ref graphics_context) = context.graphics_context {
            let config = Self {
                graphics_settings: graphics_context.renderer.get_quality_settings(),
                controls: control_scheme,
                sound: sound_config,
                show_debug_info,
            };
            let file = File::create(Self::PATH)?;
            ron::ser::to_writer_pretty(file, &config, PrettyConfig::default())?;

            Ok(())
        } else {
            Err(ConfigError::Custom("No graphics context!".to_string()))
        }
    }
}
