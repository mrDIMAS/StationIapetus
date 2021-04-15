use crate::actor::Actor;
use crate::level::turret::Turret;
use crate::{
    actor::ActorContainer,
    effects::EffectKind,
    message::Message,
    vector_to_quat,
    weapon::{ray_hit, Hit, Weapon, WeaponContainer},
    GameTime,
};
use rg3d::{
    core::{
        algebra::Vector3,
        color::Color,
        math::Vector3Ext,
        pool::{Handle, Pool, PoolIteratorMut},
        rand::Rng,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    lazy_static::lazy_static,
    physics::{
        dynamics::{BodyStatus, RigidBodyBuilder},
        geometry::ColliderBuilder,
        na::{Isometry3, Translation3},
    },
    rand,
    scene::{
        base::BaseBuilder,
        graph::Graph,
        light::{BaseLightBuilder, PointLightBuilder},
        node::Node,
        sprite::SpriteBuilder,
        ColliderHandle, RigidBodyHandle, Scene,
    },
};
use serde::Deserialize;
use std::{
    collections::HashMap, collections::HashSet, fs::File, path::PathBuf, sync::mpsc::Sender,
};

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

#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
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

impl Shooter {
    fn id(&self) -> u32 {
        match self {
            Self::None => 0,
            Self::Actor(_) => 1,
            Self::Weapon(_) => 2,
            Self::Turret(_) => 3,
        }
    }

    fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::None),
            1 => Ok(Self::Actor(Default::default())),
            2 => Ok(Self::Weapon(Default::default())),
            3 => Ok(Self::Turret(Default::default())),
            _ => Err(format!("Invalid shooter id {}!", id)),
        }
    }
}

impl Visit for Shooter {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        let mut id = self.id();
        id.visit("Id", visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(id)?;
        }
        match self {
            Shooter::None => (),
            Shooter::Actor(handle) => handle.visit("Handle", visitor)?,
            Shooter::Weapon(handle) => handle.visit("Handle", visitor)?,
            Shooter::Turret(handle) => handle.visit("Handle", visitor)?,
        }

        visitor.leave_region()
    }
}

#[derive(Deserialize, Copy, Clone, Debug)]
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
    fn id(&self) -> u32 {
        match self {
            Self::Splash { .. } => 0,
            Self::Point(_) => 1,
        }
    }

    fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::Splash {
                radius: 0.0,
                amount: 0.0,
            }),
            1 => Ok(Self::Point(0.0)),
            _ => Err(format!("Invalid damage id {}!", id)),
        }
    }

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

impl Visit for Damage {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        let mut id = self.id();
        id.visit("Id", visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(id)?;
        }
        match self {
            Damage::Splash { radius, amount } => {
                radius.visit("Radius", visitor)?;
                amount.visit("Amount", visitor)?;
            }
            Damage::Point(amount) => {
                amount.visit("Amount", visitor)?;
            }
        }

        visitor.leave_region()
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
    pub sender: Option<Sender<Message>>,
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
            sender: None,
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
        sender: Sender<Message>,
    ) -> Self {
        let definition = Self::get_definition(kind);

        let (model, body) = {
            match &kind {
                ProjectileKind::Plasma => {
                    let size = rand::thread_rng().gen_range(0.09..0.12);

                    let color = Color::opaque(0, 162, 232);
                    let model = SpriteBuilder::new(
                        BaseBuilder::new().with_children(&[PointLightBuilder::new(
                            BaseLightBuilder::new(BaseBuilder::new()).with_color(color),
                        )
                        .with_radius(1.5)
                        .build(&mut scene.graph)]),
                    )
                    .with_size(size)
                    .with_color(color)
                    .with_texture(
                        resource_manager.request_texture("data/particles/plasma_ball.png"),
                    )
                    .build(&mut scene.graph);

                    let collider = ColliderBuilder::ball(size).sensor(true).build();
                    let body = RigidBodyBuilder::new(BodyStatus::Kinematic)
                        .translation(position.x, position.y, position.z)
                        .build();
                    let body_handle = scene.physics.add_body(body);
                    scene.physics.add_collider(collider, &body_handle);
                    scene.physics_binder.bind(model, body_handle);

                    (model, Some(body_handle))
                }
                ProjectileKind::Grenade => {
                    let resource = resource_manager
                        .request_model("data/models/grenade.rgs")
                        .await
                        .unwrap();
                    let model = resource.instantiate_geometry(scene);
                    let body = scene.graph.find_by_name(model, "Body");
                    let body_handle = scene.physics_binder.body_of(body).unwrap();
                    let phys_body = scene.physics.body_mut(body_handle).unwrap();
                    phys_body.set_position(
                        Isometry3::translation(position.x, position.y, position.z),
                        true,
                    );
                    phys_body.set_linvel(initial_velocity, true);

                    (model, Some(*body_handle))
                }
            }
        };

        Self {
            lifetime: definition.lifetime,
            body,
            initial_velocity,
            dir: dir
                .try_normalize(std::f32::EPSILON)
                .unwrap_or_else(Vector3::y),
            kind,
            model,
            last_position: position,
            owner,
            definition,
            sender: Some(sender),
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
    ) {
        // Fetch current position of projectile.
        let (position, collider) = if let Some(body) = self.body.as_ref() {
            let body = scene.physics.body(body).unwrap();
            let collider: ColliderHandle = scene
                .physics
                .collider_handle_map()
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

        if let Some(hit) = ray_hit {
            self.hits.insert(hit);
            self.kill();
        }

        // Movement of kinematic projectiles are controlled explicitly.
        if self.definition.is_kinematic {
            let total_velocity = self.dir.scale(self.definition.speed);

            // Special case for projectiles with rigid body.
            if let Some(body) = self.body.as_ref() {
                // Move rigid body explicitly.
                let body = scene.physics.body_mut(body).unwrap();
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
            let (pos, normal, effect_kind) = ray_hit.map_or_else(
                || {
                    (
                        self.get_position(&scene.graph),
                        Vector3::y(),
                        EffectKind::BulletImpact,
                    )
                },
                |h| {
                    (
                        h.position,
                        h.normal,
                        if h.actor.is_some() {
                            EffectKind::BloodSpray
                        } else {
                            EffectKind::BulletImpact
                        },
                    )
                },
            );

            self.sender
                .as_ref()
                .unwrap()
                .send(Message::CreateEffect {
                    kind: effect_kind,
                    position: pos,
                    orientation: vector_to_quat(normal),
                })
                .unwrap();

            self.sender
                .as_ref()
                .unwrap()
                .send(Message::PlaySound {
                    path: PathBuf::from(self.definition.impact_sound.clone()),
                    position: pos,
                    gain: 1.0,
                    rolloff_factor: 4.0,
                    radius: 3.0,
                })
                .unwrap();
        }

        for hit in self.hits.drain() {
            let damage = self
                .definition
                .damage
                .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor));

            match damage {
                Damage::Splash { radius, amount } => {
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::ApplySplashDamage {
                            amount,
                            radius,
                            center: position,
                            who: hit.who,
                        })
                        .unwrap();
                }
                Damage::Point(amount) => {
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::DamageActor {
                            actor: hit.actor,
                            who: hit.who,
                            amount,
                        })
                        .unwrap();
                }
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

#[derive(Default)]
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
    ) {
        for projectile in self.pool.iter_mut() {
            projectile.update(scene, actors, weapons, time);
            if projectile.is_dead() {
                projectile.clean_up(scene);
            }
        }

        self.pool.retain(|proj| !proj.is_dead());
    }
}

impl Visit for ProjectileContainer {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.pool.visit("Pool", visitor)?;

        visitor.leave_region()
    }
}
