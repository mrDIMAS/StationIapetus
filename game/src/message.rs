//! Game uses message passing mechanism to perform specific actions. For example to spawn
//! a bot or item everything you need is to send appropriate message and level will create
//! required entity. This is very effective decoupling mechanism that works perfectly with
//! strict ownership rules of Rust.

use std::path::PathBuf;

#[derive(Debug)]
pub enum Message {
    Play2DSound {
        path: PathBuf,
        gain: f32,
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
