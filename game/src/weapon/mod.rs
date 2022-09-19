//! Weapon related stuff.

use crate::character::character_ref;
use crate::{
    character::{character_mut, Character, HitBox},
    current_level_mut, game_ref,
    message::Message,
    weapon::{
        definition::{WeaponDefinition, WeaponKind, WeaponProjectile},
        projectile::Shooter,
        sight::{LaserSight, SightReaction},
    },
    CollisionGroups, MessageSender,
};
use fyrox::{
    core::{
        algebra::{Matrix3, Point3, Vector3},
        inspect::prelude::*,
        math::{ray::Ray, Matrix4Ext},
        pool::Handle,
        reflect::Reflect,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    material::{shader::SamplerFallback, PropertyValue},
    rand::seq::SliceRandom,
    scene::{
        collider::{BitMask, InteractionGroups},
        graph::{
            physics::{FeatureId, Intersection, RayCastOptions},
            Graph,
        },
        node::{Node, TypeUuidProvider},
        Scene,
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
    utils::{self, log::Log},
};
use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
};

pub mod definition;
pub mod projectile;
pub mod sight;

#[derive(Debug, Default, Clone)]
pub struct ShotRequest {
    direction: Option<Vector3<f32>>,
}

#[derive(Visit, Reflect, Inspect, Debug, Clone)]
pub struct Weapon {
    kind: WeaponKind,
    shot_point: Handle<Node>,
    muzzle_flash: Handle<Node>,
    shot_light: Handle<Node>,
    shot_position: Vector3<f32>,
    muzzle_flash_timer: f32,
    flash_light: Handle<Node>,
    flash_light_enabled: bool,
    pub enabled: bool,

    #[reflect(hidden)]
    #[inspect(skip)]
    laser_sight: LaserSight,

    #[reflect(hidden)]
    #[inspect(skip)]
    owner: Handle<Node>,

    #[reflect(hidden)]
    #[inspect(skip)]
    #[visit(optional)]
    last_shot_time: f32,

    #[reflect(hidden)]
    #[inspect(skip)]
    #[visit(skip)]
    pub definition: &'static WeaponDefinition,

    #[reflect(hidden)]
    #[inspect(skip)]
    #[visit(skip)]
    shot_request: Option<ShotRequest>,

    #[reflect(hidden)]
    #[inspect(skip)]
    #[visit(skip)]
    self_handle: Handle<Node>,
}

impl Default for Weapon {
    fn default() -> Self {
        Self {
            kind: WeaponKind::M4,
            shot_point: Handle::NONE,
            last_shot_time: 0.0,
            shot_position: Vector3::default(),
            owner: Handle::NONE,
            muzzle_flash_timer: 0.0,
            definition: Self::definition(WeaponKind::M4),
            muzzle_flash: Default::default(),
            shot_light: Default::default(),
            flash_light: Default::default(),
            flash_light_enabled: false,
            enabled: true,
            laser_sight: Default::default(),
            shot_request: None,
            self_handle: Default::default(),
        }
    }
}

#[derive(Clone)]
pub struct Hit {
    pub actor: Handle<Node>, // Can be None if level geometry was hit.
    pub who: Handle<Node>,
    pub position: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub collider: Handle<Node>,
    pub feature: FeatureId,
    pub hit_box: Option<HitBox>,
    pub query_buffer: Vec<Intersection>,
}

impl PartialEq for Hit {
    fn eq(&self, other: &Self) -> bool {
        self.actor == other.actor
            && self.who == other.who
            && self.position == other.position
            && self.normal == other.normal
            && self.collider == other.collider
            && self.feature == other.feature
            && self.hit_box == other.hit_box
    }
}

impl Hash for Hit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        utils::hash_as_bytes(self, state);
    }
}

impl Eq for Hit {}

/// Checks intersection of given ray with actors and environment.
pub fn ray_hit(
    begin: Vector3<f32>,
    end: Vector3<f32>,
    shooter: Shooter,
    actors: &[Handle<Node>],
    graph: &mut Graph,
    ignored_collider: Handle<Node>,
) -> Option<Hit> {
    let physics = &mut graph.physics;
    let ray = Ray::from_two_points(begin, end);

    // TODO: Avoid allocation.
    let mut query_buffer = Vec::default();

    physics.cast_ray(
        RayCastOptions {
            ray_origin: Point3::from(ray.origin),
            ray_direction: ray.dir,
            max_len: ray.dir.norm(),
            groups: InteractionGroups::new(
                BitMask(0xFFFF),
                BitMask(!(CollisionGroups::ActorCapsule as u32)),
            ),
            sort_results: true,
        },
        &mut query_buffer,
    );

    // List of hits sorted by distance from ray origin.
    if let Some(hit) = query_buffer.iter().find(|i| i.collider != ignored_collider) {
        let mut is_hitbox_hit = false;

        // Check if there was an intersection with an actor.
        'actor_loop: for &actor_handle in actors.iter() {
            let character = character_ref(actor_handle, graph);
            for hit_box in character.hit_boxes.iter() {
                if hit_box.collider == hit.collider {
                    is_hitbox_hit = true;

                    let who = match shooter {
                        Shooter::None | Shooter::Turret(_) => Default::default(),
                        Shooter::Actor(actor) => actor,
                        Shooter::Weapon(weapon) => Default::default(), //weapon_ref(weapon, graph).owner(), TODO
                    };

                    // Ignore intersections with owners.
                    if who == actor_handle {
                        continue 'actor_loop;
                    }

                    return Some(Hit {
                        actor: actor_handle,
                        who,
                        position: hit.position.coords,
                        normal: hit.normal,
                        collider: hit.collider,
                        feature: hit.feature,
                        hit_box: Some(*hit_box),
                        query_buffer,
                    });
                }
            }
        }

        if is_hitbox_hit {
            None
        } else {
            Some(Hit {
                actor: Handle::NONE,
                who: Handle::NONE,
                position: hit.position.coords,
                normal: hit.normal,
                collider: hit.collider,
                feature: hit.feature,
                hit_box: None,
                query_buffer,
            })
        }
    } else {
        None
    }
}

impl Weapon {
    pub fn definition(kind: WeaponKind) -> &'static WeaponDefinition {
        definition::DEFINITIONS.map.get(&kind).unwrap()
    }

    pub fn shot_position(&self, graph: &Graph) -> Vector3<f32> {
        if self.shot_point.is_some() {
            graph[self.shot_point].global_position()
        } else {
            // Fallback
            graph[self.self_handle].global_position()
        }
    }

    pub fn shot_direction(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.self_handle].look_vector().normalize()
    }

    pub fn kind(&self) -> WeaponKind {
        self.kind
    }

    pub fn world_basis(&self, graph: &Graph) -> Matrix3<f32> {
        graph[self.self_handle].global_transform().basis()
    }

    pub fn owner(&self) -> Handle<Node> {
        self.owner
    }

    pub fn set_owner(&mut self, owner: Handle<Node>) {
        self.owner = owner;
    }

    pub fn switch_flash_light(&mut self) {
        self.flash_light_enabled = !self.flash_light_enabled;
    }

    pub fn laser_sight(&self) -> &LaserSight {
        &self.laser_sight
    }

    pub fn laser_sight_mut(&mut self) -> &mut LaserSight {
        &mut self.laser_sight
    }

    pub fn can_shoot(&self, elapsed_time: f32) -> bool {
        elapsed_time - self.last_shot_time >= self.definition.shoot_interval
    }

    pub fn set_sight_reaction(&mut self, reaction: SightReaction) {
        self.laser_sight.set_reaction(reaction);
    }

    pub fn request_shot(&mut self, direction: Option<Vector3<f32>>) {
        self.shot_request = Some(ShotRequest { direction });
    }

    fn shoot(
        &mut self,
        self_handle: Handle<Node>,
        scene: &mut Scene,
        elapsed_time: f32,
        resource_manager: ResourceManager,
        direction: Option<Vector3<f32>>,
        sender: &MessageSender,
    ) {
        self.last_shot_time = elapsed_time;

        let position = self.shot_position(&scene.graph);

        if let Some(random_shot_sound) = self
            .definition
            .shot_sounds
            .choose(&mut fyrox::rand::thread_rng())
        {
            sender.send(Message::PlaySound {
                path: PathBuf::from(random_shot_sound.clone()),
                position,
                gain: 1.0,
                rolloff_factor: 5.0,
                radius: 3.0,
            });
        }

        if self.muzzle_flash.is_some() {
            let muzzle_flash = &mut scene.graph[self.muzzle_flash];
            muzzle_flash.set_visibility(true);
            for surface in muzzle_flash.as_mesh_mut().surfaces_mut() {
                let textures = [
                    "data/particles/muzzle_01.png",
                    "data/particles/muzzle_02.png",
                    "data/particles/muzzle_03.png",
                    "data/particles/muzzle_04.png",
                    "data/particles/muzzle_05.png",
                ];
                Log::verify(surface.material().lock().set_property(
                    &ImmutableString::new("diffuseTexture"),
                    PropertyValue::Sampler {
                        value: Some(resource_manager.request_texture(
                            textures.choose(&mut fyrox::rand::thread_rng()).unwrap(),
                        )),
                        fallback: SamplerFallback::White,
                    },
                ));
            }
            scene.graph[self.shot_light].set_visibility(true);
            self.muzzle_flash_timer = 0.075;
        }

        let position = self.shot_position(&scene.graph);
        let direction = direction
            .unwrap_or_else(|| self.shot_direction(&scene.graph))
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        match self.definition.projectile {
            WeaponProjectile::Projectile(projectile) => sender.send(Message::CreateProjectile {
                kind: projectile,
                position,
                direction,
                shooter: Shooter::Weapon(self_handle),
                initial_velocity: Default::default(),
            }),
            WeaponProjectile::Ray { damage } => {
                sender.send(Message::ShootRay {
                    shooter: Shooter::Weapon(self_handle),
                    begin: position,
                    end: position + direction.scale(1000.0),
                    damage,
                    shot_effect: self.definition.shot_effect,
                });
            }
        }
    }
}

impl_component_provider!(Weapon);

impl TypeUuidProvider for Weapon {
    fn type_uuid() -> Uuid {
        uuid!("bca0083b-b062-4d95-b241-db05bca65da7")
    }
}

impl ScriptTrait for Weapon {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        self.definition = Self::definition(self.kind);
        self.self_handle = ctx.handle;
        self.laser_sight = LaserSight::new(ctx.scene, ctx.resource_manager.clone());

        dbg!(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        if let Some(level) = current_level_mut(ctx.plugins) {
            for projectile in level.projectiles.iter_mut() {
                if let Shooter::Weapon(ref mut owner) = projectile.owner {
                    // Reset owner because handle to weapon will be invalid after weapon freed.
                    if *owner == ctx.node_handle {
                        *owner = Handle::NONE;
                    }
                }
            }

            for actor in level.actors.iter() {
                let character = character_mut(*actor, &mut ctx.scene.graph);

                if let Some(i) = character.weapons.iter().position(|&w| w == ctx.node_handle) {
                    character.weapons.remove(i);
                }

                if character.current_weapon() == ctx.node_handle {
                    if let Some(&first_weapon) = character.weapons.first() {
                        character.current_weapon = 0;
                        weapon_mut(first_weapon, &mut ctx.scene.graph).enabled = true;
                    }
                }
            }
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        ctx.scene.graph[ctx.handle].set_visibility(self.enabled);

        let node = &mut ctx.scene.graph[ctx.handle];
        self.shot_position = node.global_position();

        self.muzzle_flash_timer -= ctx.dt;
        if self.muzzle_flash_timer <= 0.0 && self.muzzle_flash.is_some() {
            ctx.scene.graph[self.muzzle_flash].set_visibility(false);
            ctx.scene.graph[self.shot_light].set_visibility(false);
        }

        let mut ignored_collider = Default::default();

        if let Some(node) = ctx.scene.graph.try_get(self.owner) {
            if let Some(character) = node
                .script()
                .and_then(|s| s.query_component_ref::<Character>())
            {
                ignored_collider = character.capsule_collider;
            }
        }

        let dir = self.shot_direction(&ctx.scene.graph);
        let pos = self.shot_position(&ctx.scene.graph);
        self.laser_sight
            .update(ctx.scene, pos, dir, ignored_collider, ctx.dt);

        if let Some(flash_light) = ctx.scene.graph.try_get_mut(self.flash_light) {
            flash_light.set_visibility(self.flash_light_enabled);
        }

        if let Some(request) = self.shot_request.take() {
            let game = game_ref(ctx.plugins);
            self.shoot(
                ctx.handle,
                ctx.scene,
                ctx.elapsed_time,
                ctx.resource_manager.clone(),
                request.direction,
                &game.message_sender,
            );
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

pub fn weapon_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Weapon {
    graph[handle].try_get_script_mut::<Weapon>().unwrap()
}

pub fn try_weapon_ref(handle: Handle<Node>, graph: &Graph) -> Option<&Weapon> {
    graph[handle].try_get_script::<Weapon>()
}

pub fn weapon_ref(handle: Handle<Node>, graph: &Graph) -> &Weapon {
    try_weapon_ref(handle, graph).unwrap()
}
