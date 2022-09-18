//! Game uses message passing mechanism to perform specific actions. For example to spawn
//! a bot or item everything you need is to send appropriate message and level will create
//! required entity. This is very effective decoupling mechanism that works perfectly with
//! strict ownership rules of Rust.

use crate::{
    actor::Actor,
    bot::BotKind,
    character::HitBox,
    effects::EffectKind,
    elevator::{call_button::CallButton, Elevator},
    sound::SoundKind,
    weapon::{
        definition::ShotEffect,
        projectile::{Damage, ProjectileKind, Shooter},
    },
};
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        pool::Handle,
    },
    scene::{graph::physics::FeatureId, node::Node},
};
use std::path::PathBuf;

#[derive(Debug)]
pub enum Message {
    SetCallButtonFloor {
        call_button: Handle<CallButton>,
        floor: u32,
    },
    CallElevator {
        elevator: Handle<Elevator>,
        floor: u32,
    },
    TryOpenDoor {
        door: Handle<Node>,
        actor: Handle<Actor>,
    },
    AddBot {
        kind: BotKind,
        position: Vector3<f32>,
        rotation: UnitQuaternion<f32>,
    },
    RemoveActor {
        actor: Handle<Actor>,
    },
    SpawnBot {
        spawn_point_id: usize,
    },
    CreateProjectile {
        kind: ProjectileKind,
        position: Vector3<f32>,
        direction: Vector3<f32>,
        initial_velocity: Vector3<f32>,
        shooter: Shooter,
    },
    ShootRay {
        shooter: Shooter,
        begin: Vector3<f32>,
        end: Vector3<f32>,
        damage: Damage,
        shot_effect: ShotEffect,
    },
    PlaySound {
        path: PathBuf,
        position: Vector3<f32>,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    },
    Play2DSound {
        path: PathBuf,
        gain: f32,
    },
    /// Plays environment-specific sound. It also handles foot step sounds.
    PlayEnvironmentSound {
        collider: Handle<Node>,
        feature: FeatureId,
        position: Vector3<f32>,
        sound_kind: SoundKind,
        gain: f32,
        rolloff_factor: f32,
        radius: f32,
    },
    DamageActor {
        /// Which actor should be damaged.
        actor: Handle<Actor>,
        /// Actor who damaged target actor, can be Handle::NONE if damage came from environment
        /// or not from any actor.
        who: Handle<Actor>,
        /// A body part which was hit.
        hitbox: Option<HitBox>,
        /// Numeric value of damage.
        amount: f32,
        /// Only takes effect iff damage was applied to a head hit box!
        critical_shot_probability: f32,
    },
    CreateEffect {
        kind: EffectKind,
        position: Vector3<f32>,
        orientation: UnitQuaternion<f32>,
    },
    ApplySplashDamage {
        amount: f32,
        radius: f32,
        center: Vector3<f32>,
        /// Damage initiator
        who: Handle<Actor>,
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
