use crate::{
    actor::{Actor, ActorContainer},
    effects::EffectKind,
    level::turret::Turret,
    message::Message,
    weapon::{ray_hit, sight::SightReaction, Hit, Weapon, WeaponContainer},
    GameTime, MessageSender,
};
use rg3d::{
    core::{
        algebra::{Isometry3, Translation3, Vector3},
        math::{vector_to_quat, Vector3Ext},
        pool::{Handle, Pool, PoolIteratorMut},
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::{MaterialSearchOptions, ResourceManager},
    lazy_static::lazy_static,
    physics3d::{ColliderHandle, RigidBodyHandle},
    scene::{graph::Graph, node::Node, Scene},
};
use serde::Deserialize;
use std::{collections::HashMap, collections::HashSet, fs::File, path::PathBuf};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Hash)]
pub enum ProjectileKind {
    Plasma,
    Grenade,
}

impl ProjectileKind {
    pub fn new(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(ProjectileKind::Plasma),
            1 => Ok(ProjectileKind::Grenade),
            _ => Err(format!("Invalid projectile kind id {}", id)),
        }
    }

    pub fn id(self) -> u32 {
        match self {
            ProjectileKind::Plasma => 0,
            ProjectileKind::Grenade => 1,
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Visit)]
pub enum Shooter {
    None,
    Actor(Handle<Actor>),
    Weapon(Handle<Weapon>),
    Turret(Handle<Turret>),
}

impl Default for Shooter {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Deserialize, Copy, Clone, Debug, Visit)]
pub enum Damage {
    Splash { radius: f32, amount: f32 },
    Point(f32),
}

impl Default for Damage {
    fn default() -> Self {
        Self::Point(0.0)
    }
}

impl Damage {
    #[must_use]
    pub fn scale(&self, k: f32) -> Self {
        match *self {
            Self::Splash { amount, radius } => Self::Splash {
                amount: amount * k.abs(),
                radius,
            },
            Self::Point(amount) => Self::Point(amount * k.abs()),
        }
    }

    pub fn amount(&self) -> f32 {
        *match self {
            Damage::Splash { amount, .. } => amount,
            Damage::Point(amount) => amount,
        }
    }
}

pub struct Projectile {
    kind: ProjectileKind,
    model: Handle<Node>,
    /// Handle of rigid body assigned to projectile. Some projectiles, like grenades,
    /// rockets, plasma balls could have rigid body to detect collisions with
    /// environment. Some projectiles do not have rigid body - they're ray-based -
    /// interaction with environment handled with ray cast.
    body: Option<RigidBodyHandle>,
    dir: Vector3<f32>,
    lifetime: f32,
    rotation_angle: f32,
    pub owner: Shooter,
    initial_velocity: Vector3<f32>,
    /// Position of projectile on the previous frame, it is used to simulate
    /// continuous intersection detection from fast moving projectiles.
    last_position: Vector3<f32>,
    definition: &'static ProjectileDefinition,
    hits: HashSet<Hit>,
}

impl Default for Projectile {
    fn default() -> Self {
        Self {
            kind: ProjectileKind::Plasma,
            model: Default::default(),
            dir: Default::default(),
            body: Default::default(),
            lifetime: 0.0,
            rotation_angle: 0.0,
            owner: Default::default(),
            initial_velocity: Default::default(),
            last_position: Default::default(),
            definition: Self::get_definition(ProjectileKind::Plasma),
            hits: Default::default(),
        }
    }
}

#[derive(Deserialize)]
pub struct ProjectileDefinition {
    damage: Damage,
    speed: f32,
    lifetime: f32,
    /// Means that movement of projectile controlled by code, not physics.
    /// However projectile still could have rigid body to detect collisions.
    is_kinematic: bool,
    impact_sound: String,
    model: String,
}

#[derive(Deserialize, Default)]
pub struct ProjectileDefinitionContainer {
    map: HashMap<ProjectileKind, ProjectileDefinition>,
}

impl ProjectileDefinitionContainer {
    pub fn new() -> Self {
        let file = File::open("data/configs/projectiles.ron").unwrap();
        ron::de::from_reader(file).unwrap()
    }
}

lazy_static! {
    static ref DEFINITIONS: ProjectileDefinitionContainer = ProjectileDefinitionContainer::new();
}

impl Projectile {
    pub fn get_definition(kind: ProjectileKind) -> &'static ProjectileDefinition {
        DEFINITIONS.map.get(&kind).unwrap()
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        kind: ProjectileKind,
        resource_manager: ResourceManager,
        scene: &mut Scene,
        dir: Vector3<f32>,
        position: Vector3<f32>,
        owner: Shooter,
        initial_velocity: Vector3<f32>,
    ) -> Self {
        let definition = Self::get_definition(kind);

        let resource = resource_manager
            .request_model(
                definition.model.clone(),
                MaterialSearchOptions::UsePathDirectly,
            )
            .await
            .unwrap();
        let model = resource.instantiate_geometry(scene);
        let body = scene.graph.find_by_name(model, "Projectile");
        let body_handle = scene.physics_binder.body_of(body).unwrap();
        let phys_body = scene.physics.bodies.get_mut(body_handle).unwrap();
        phys_body.set_position(
            Isometry3::translation(position.x, position.y, position.z),
            true,
        );
        phys_body.set_linvel(initial_velocity, true);

        Self {
            lifetime: definition.lifetime,
            body: Some(*body_handle),
            initial_velocity,
            dir: dir
                .try_normalize(std::f32::EPSILON)
                .unwrap_or_else(Vector3::y),
            kind,
            model,
            last_position: position,
            owner,
            definition,
            ..Default::default()
        }
    }

    pub fn is_dead(&self) -> bool {
        self.lifetime <= 0.0
    }

    pub fn kill(&mut self) {
        self.lifetime = 0.0;
    }

    pub fn update(
        &mut self,
        scene: &mut Scene,
        actors: &ActorContainer,
        weapons: &WeaponContainer,
        time: GameTime,
        sender: &MessageSender,
    ) {
        // Fetch current position of projectile.
        let (position, collider) = if let Some(body) = self.body.as_ref() {
            let body = scene.physics.bodies.get(body).unwrap();
            let collider: ColliderHandle = scene
                .physics
                .colliders
                .handle_map()
                .key_of(body.colliders().first().unwrap())
                .cloned()
                .unwrap();
            (body.position().translation.vector, collider)
        } else {
            (
                scene.graph[self.model].global_position(),
                ColliderHandle::default(),
            )
        };

        let ray_hit = ray_hit(
            self.last_position,
            position,
            self.owner,
            weapons,
            actors,
            &mut scene.physics,
            collider,
        );

        let (effect_position, effect_normal, effect_kind) = if let Some(hit) = ray_hit {
            let position = hit.position;
            let normal = hit.normal;
            let blood_effect = hit.actor.is_some();

            self.hits.insert(hit);
            self.kill();

            (
                position,
                normal,
                if blood_effect {
                    EffectKind::BloodSpray
                } else {
                    EffectKind::BulletImpact
                },
            )
        } else {
            (
                self.get_position(&scene.graph),
                Vector3::y(),
                EffectKind::BulletImpact,
            )
        };

        // Movement of kinematic projectiles are controlled explicitly.
        if self.definition.is_kinematic {
            let total_velocity = self.dir.scale(self.definition.speed);

            // Special case for projectiles with rigid body.
            if let Some(body) = self.body.as_ref() {
                // Move rigid body explicitly.
                let body = scene.physics.bodies.get_mut(body).unwrap();
                let position = Isometry3 {
                    rotation: Default::default(),
                    translation: Translation3 {
                        vector: body.position().translation.vector + total_velocity,
                    },
                };
                body.set_next_kinematic_position(position);
            } else {
                // We have just model - move it.
                scene.graph[self.model]
                    .local_transform_mut()
                    .offset(total_velocity);
            }
        }

        if let Node::Sprite(sprite) = &mut scene.graph[self.model] {
            sprite.set_rotation(self.rotation_angle);
            self.rotation_angle += 1.5;
        }

        // Reduce initial velocity down to zero over time. This is needed because projectile
        // stabilizes its movement over time.
        self.initial_velocity.follow(&Vector3::default(), 0.15);

        self.lifetime -= time.delta;

        if self.lifetime <= 0.0 {
            sender.send(Message::CreateEffect {
                kind: effect_kind,
                position: effect_position,
                orientation: vector_to_quat(effect_normal),
            });

            sender.send(Message::PlaySound {
                path: PathBuf::from(self.definition.impact_sound.clone()),
                position: effect_position,
                gain: 1.0,
                rolloff_factor: 4.0,
                radius: 3.0,
            });
        }

        for hit in self.hits.drain() {
            let damage = self
                .definition
                .damage
                .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor));

            let critical_shot_probability = match self.owner {
                Shooter::Weapon(weapon) => {
                    if hit.actor.is_some() {
                        sender.send(Message::SightReaction {
                            weapon,
                            reaction: SightReaction::HitDetected,
                        });
                    }

                    weapons[weapon].definition.base_critical_shot_probability
                }
                Shooter::Turret(_) => 0.01,
                _ => 0.0,
            };

            match damage {
                Damage::Splash { radius, amount } => sender.send(Message::ApplySplashDamage {
                    amount,
                    radius,
                    center: position,
                    who: hit.who,
                    critical_shot_probability,
                }),
                Damage::Point(amount) => sender.send(Message::DamageActor {
                    actor: hit.actor,
                    who: hit.who,
                    hitbox: hit.hit_box,
                    amount,
                    critical_shot_probability,
                }),
            }
        }

        self.last_position = position;
    }

    pub fn get_position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.model].global_position()
    }

    fn clean_up(&mut self, scene: &mut Scene) {
        if let Some(body) = self.body.as_ref() {
            scene.physics.remove_body(body);
        }
        if self.model.is_some() {
            scene.graph.remove_node(self.model);
        }
    }
}

impl Visit for Projectile {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        let mut kind = self.kind.id();
        kind.visit("KindId", visitor)?;
        if visitor.is_reading() {
            self.kind = ProjectileKind::new(kind)?;
        }

        self.definition = Self::get_definition(self.kind);
        self.lifetime.visit("Lifetime", visitor)?;
        self.dir.visit("Direction", visitor)?;
        self.model.visit("Model", visitor)?;
        self.body.visit("Body", visitor)?;
        self.rotation_angle.visit("RotationAngle", visitor)?;
        self.initial_velocity.visit("InitialVelocity", visitor)?;
        self.owner.visit("Owner", visitor)?;

        visitor.leave_region()
    }
}

#[derive(Default, Visit)]
pub struct ProjectileContainer {
    pool: Pool<Projectile>,
}

impl ProjectileContainer {
    pub fn new() -> Self {
        Self { pool: Pool::new() }
    }

    pub fn add(&mut self, projectile: Projectile) -> Handle<Projectile> {
        self.pool.spawn(projectile)
    }

    pub fn iter_mut(&mut self) -> PoolIteratorMut<Projectile> {
        self.pool.iter_mut()
    }

    pub fn update(
        &mut self,
        scene: &mut Scene,
        actors: &ActorContainer,
        weapons: &WeaponContainer,
        time: GameTime,
        sender: &MessageSender,
    ) {
        for projectile in self.pool.iter_mut() {
            projectile.update(scene, actors, weapons, time, sender);
            if projectile.is_dead() {
                projectile.clean_up(scene);
            }
        }

        self.pool.retain(|proj| !proj.is_dead());
    }
}
