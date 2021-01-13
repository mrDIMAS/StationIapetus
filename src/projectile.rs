use crate::{
    actor::{Actor, ActorContainer},
    effects::EffectKind,
    message::Message,
    weapon::{Weapon, WeaponContainer},
    GameTime,
};
use rg3d::{
    core::rand::Rng,
    core::{
        algebra::{Matrix3, UnitQuaternion, Vector3},
        color::Color,
        math::{ray::Ray, Vector3Ext},
        pool::{Handle, Pool, PoolIteratorMut},
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    physics::{
        dynamics::{BodyStatus, RigidBodyBuilder},
        geometry::{ColliderBuilder, InteractionGroups, Proximity, ProximityEvent},
        na::{Isometry3, Translation3},
    },
    rand,
    scene::{
        base::BaseBuilder,
        graph::Graph,
        light::{BaseLightBuilder, PointLightBuilder},
        node::Node,
        physics::RayCastOptions,
        sprite::SpriteBuilder,
        transform::TransformBuilder,
        RigidBodyHandle, Scene,
    },
};
use std::{collections::HashSet, path::PathBuf, sync::mpsc::Sender};

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
pub enum ProjectileKind {
    Plasma,
    Bullet,
    Rocket,
    Grenade,
}

impl ProjectileKind {
    pub fn new(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(ProjectileKind::Plasma),
            1 => Ok(ProjectileKind::Bullet),
            2 => Ok(ProjectileKind::Rocket),
            3 => Ok(ProjectileKind::Grenade),
            _ => Err(format!("Invalid projectile kind id {}", id)),
        }
    }

    pub fn id(self) -> u32 {
        match self {
            ProjectileKind::Plasma => 0,
            ProjectileKind::Bullet => 1,
            ProjectileKind::Rocket => 2,
            ProjectileKind::Grenade => 3,
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
    body: RigidBodyHandle,
    dir: Vector3<f32>,
    lifetime: f32,
    rotation_angle: f32,
    /// Handle of weapons from which projectile was fired.
    pub owner: Handle<Weapon>,
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

pub struct ProjectileDefinition {
    damage: f32,
    speed: f32,
    lifetime: f32,
    /// Means that movement of projectile controlled by code, not physics.
    /// However projectile still could have rigid body to detect collisions.
    is_kinematic: bool,
    impact_sound: &'static str,
}

impl Projectile {
    pub fn get_definition(kind: ProjectileKind) -> &'static ProjectileDefinition {
        match kind {
            ProjectileKind::Plasma => {
                static DEFINITION: ProjectileDefinition = ProjectileDefinition {
                    damage: 30.0,
                    speed: 0.15,
                    lifetime: 10.0,
                    is_kinematic: true,
                    impact_sound: "data/sounds/bullet_impact_concrete.ogg",
                };
                &DEFINITION
            }
            ProjectileKind::Bullet => {
                static DEFINITION: ProjectileDefinition = ProjectileDefinition {
                    damage: 15.0,
                    speed: 0.75,
                    lifetime: 10.0,
                    is_kinematic: true,
                    impact_sound: "data/sounds/bullet_impact_concrete.ogg",
                };
                &DEFINITION
            }
            ProjectileKind::Rocket => {
                static DEFINITION: ProjectileDefinition = ProjectileDefinition {
                    damage: 60.0,
                    speed: 0.5,
                    lifetime: 10.0,
                    is_kinematic: true,
                    impact_sound: "data/sounds/explosion.ogg",
                };
                &DEFINITION
            }
            ProjectileKind::Grenade => {
                static DEFINITION: ProjectileDefinition = ProjectileDefinition {
                    damage: 60.0,
                    speed: 0.0,
                    lifetime: 10.0,
                    is_kinematic: false,
                    impact_sound: "data/sounds/explosion.ogg",
                };
                &DEFINITION
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        kind: ProjectileKind,
        resource_manager: ResourceManager,
        scene: &mut Scene,
        dir: Vector3<f32>,
        position: Vector3<f32>,
        owner: Handle<Weapon>,
        initial_velocity: Vector3<f32>,
        sender: Sender<Message>,
        basis: Matrix3<f32>,
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
                    .with_texture(resource_manager.request_texture("data/particles/light_01.png"))
                    .build(&mut scene.graph);

                    let collider = ColliderBuilder::ball(size).sensor(true).build();
                    let body = RigidBodyBuilder::new(BodyStatus::Kinematic)
                        .translation(position.x, position.y, position.z)
                        .build();
                    let body_handle = scene.physics.add_body(body);
                    scene.physics.add_collider(collider, body_handle);
                    scene.physics_binder.bind(model, body_handle);

                    (model, body_handle)
                }
                ProjectileKind::Bullet => {
                    let model = SpriteBuilder::new(
                        BaseBuilder::new().with_local_transform(
                            TransformBuilder::new()
                                .with_local_position(position)
                                .build(),
                        ),
                    )
                    .with_size(0.05)
                    .with_texture(resource_manager.request_texture("data/particles/light_01.png"))
                    .build(&mut scene.graph);

                    (model, Default::default())
                }
                ProjectileKind::Rocket => {
                    let resource = resource_manager
                        .request_model("data/models/rocket.FBX")
                        .await
                        .unwrap();
                    let model = resource.instantiate_geometry(scene);
                    scene.graph[model]
                        .local_transform_mut()
                        .set_rotation(UnitQuaternion::from_matrix(&basis))
                        .set_position(position);
                    let light = PointLightBuilder::new(
                        BaseLightBuilder::new(BaseBuilder::new())
                            .with_color(Color::opaque(255, 127, 0)),
                    )
                    .with_radius(1.5)
                    .build(&mut scene.graph);
                    scene.graph.link_nodes(light, model);
                    (model, Default::default())
                }
                ProjectileKind::Grenade => {
                    let resource = resource_manager
                        .request_model("data/models/grenade.rgs")
                        .await
                        .unwrap();
                    let model = resource.instantiate_geometry(scene);
                    let body = scene.graph.find_by_name(model, "Body");
                    let body_handle = scene.physics_binder.body_of(body).unwrap();
                    let phys_body = scene.physics.bodies.get_mut(body_handle.into()).unwrap();
                    phys_body.set_position(
                        Isometry3::translation(position.x, position.y, position.z),
                        true,
                    );
                    phys_body.set_linvel(initial_velocity, true);

                    (model, body_handle)
                }
            }
        };

        Self {
            lifetime: definition.lifetime,
            body,
            initial_velocity,
            dir: dir.try_normalize(std::f32::EPSILON).unwrap_or(Vector3::y()),
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
        let position = if self.body.is_some() {
            scene
                .physics
                .bodies
                .get(self.body.into())
                .unwrap()
                .position()
                .translation
                .vector
        } else {
            scene.graph[self.model].global_position()
        };

        let mut effect_position = None;

        // Do ray based intersection tests for every kind of projectiles. This will help to handle
        // fast moving projectiles.
        if let Some(ray) = Ray::from_two_points(&self.last_position, &position) {
            let mut query_buffer = Vec::default();
            scene.physics.cast_ray(
                RayCastOptions {
                    ray,
                    max_len: ray.dir.norm(),
                    groups: InteractionGroups::all(),
                    sort_results: true,
                },
                &mut query_buffer,
            );

            // List of hits sorted by distance from ray origin.
            'hit_loop: for hit in query_buffer.iter() {
                let collider = scene.physics.colliders.get(hit.collider.into()).unwrap();
                let body = collider.parent();

                if collider.shape().as_trimesh().is_some() {
                    self.kill();
                    effect_position = Some(hit.position.coords);
                    break 'hit_loop;
                } else {
                    for (actor_handle, actor) in actors.pair_iter() {
                        if actor.get_body() == body.into() && self.owner.is_some() {
                            let weapon = &weapons[self.owner];
                            // Ignore intersections with owners of weapon.
                            if weapon.owner() != actor_handle {
                                self.hits.insert(Hit {
                                    actor: actor_handle,
                                    who: weapon.owner(),
                                });

                                self.kill();
                                effect_position = Some(hit.position.coords);
                                break 'hit_loop;
                            }
                        }
                    }
                }
            }
        }

        // Movement of kinematic projectiles are controlled explicitly.
        if self.definition.is_kinematic {
            let total_velocity = self.dir.scale(self.definition.speed);

            // Special case for projectiles with rigid body.
            if self.body.is_some() {
                // Move rigid body explicitly.
                let body = scene.physics.bodies.get_mut(self.body.into()).unwrap();
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
            let pos = effect_position.unwrap_or_else(|| self.get_position(&scene.graph));

            self.sender
                .as_ref()
                .unwrap()
                .send(Message::CreateEffect {
                    kind: EffectKind::BulletImpact,
                    position: pos,
                })
                .unwrap();

            self.sender
                .as_ref()
                .unwrap()
                .send(Message::PlaySound {
                    path: PathBuf::from(self.definition.impact_sound),
                    position: pos,
                    gain: 1.0,
                    rolloff_factor: 4.0,
                    radius: 3.0,
                })
                .unwrap();
        }

        for hit in self.hits.drain() {
            self.sender
                .as_ref()
                .unwrap()
                .send(Message::DamageActor {
                    actor: hit.actor,
                    who: hit.who,
                    amount: self.definition.damage,
                })
                .unwrap();
        }

        self.last_position = position;
    }

    /// Some projectiles have just proximity sensors which used to detect contacts with
    /// environment and actors. We have to handle proximity events separately.
    pub fn handle_proximity(
        &mut self,
        proximity_event: &ProximityEvent,
        scene: &mut Scene,
        actors: &ActorContainer,
        weapons: &WeaponContainer,
    ) {
        if proximity_event.new_status == Proximity::Intersecting
            || proximity_event.new_status == Proximity::WithinMargin
        {
            let mut owner_contact = false;

            let body_a = scene
                .physics
                .colliders
                .get(proximity_event.collider1)
                .unwrap()
                .parent();
            let body_b = scene
                .physics
                .colliders
                .get(proximity_event.collider2)
                .unwrap()
                .parent();

            // Check if we got contact with any actor and damage it then.
            for (actor_handle, actor) in actors.pair_iter() {
                if (body_a == actor.get_body().into() && body_b == self.body.into()
                    || body_b == actor.get_body().into() && body_a == self.body.into())
                    && self.owner.is_some()
                {
                    // Prevent self-damage.
                    let weapon = &weapons[self.owner];
                    if weapon.owner() != actor_handle {
                        self.hits.insert(Hit {
                            actor: actor_handle,
                            who: weapon.owner(),
                        });
                    } else {
                        // Make sure that projectile won't die on contact with owner.
                        owner_contact = true;
                    }
                }
            }

            if !owner_contact {
                self.kill();
            }
        }
    }

    pub fn get_position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.model].global_position()
    }

    fn clean_up(&mut self, scene: &mut Scene) {
        if self.body.is_some() {
            scene.physics.remove_body(self.body);
        }
        if self.model.is_some() {
            scene.graph.remove_node(self.model);
        }
    }
}

#[derive(Hash, Eq, PartialEq)]
struct Hit {
    actor: Handle<Actor>,
    who: Handle<Actor>,
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
