//! Game uses message passing mechanism to perform specific actions. For example to spawn
//! a bot or item everything you need is to send appropriate message and level will create
//! required entity. This is very effective decoupling mechanism that works perfectly with
//! strict ownership rules of Rust.

use fyrox::{
    core::{algebra::Vector3, pool::Handle},
    scene::node::Node,
};
use std::path::PathBuf;

#[derive(Debug)]
pub enum Message {
    Play2DSound {
        path: PathBuf,
        gain: f32,
    },
    ApplySplashDamage {
        amount: f32,
        radius: f32,
        center: Vector3<f32>,
        /// Damage initiator
        who: Handle<Node>,
        critical_shot_probability: f32,
    },
    /// Save game state to a file. TODO: Add filename field.
    SaveGame,
    /// Loads game state from a file. TODO: Add filename field.
    LoadGame,
    StartNewGame,
    LoadTestbed,
    QuitGame,
    LoadNextLevel,
    ToggleMainMenu,
    EndMatch,
    EndGame,
    SyncInventory,
    SyncJournal,
    SaveConfig,
    // Sound-related messages.
    SetMusicVolume(f32),
    SetUseHrtf(bool),
    SetMasterVolume(f32),
}
