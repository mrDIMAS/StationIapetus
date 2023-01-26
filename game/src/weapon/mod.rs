//! Weapon related stuff.

use crate::{
    character::{character_ref, HitBox},
    current_level_ref,
    sound::SoundManager,
    weapon::{
        definition::{WeaponDefinition, WeaponKind},
        projectile::Projectile,
    },
    CollisionGroups,
};
use fyrox::{
    core::{
        algebra::{Matrix3, Point3, Vector3},
        math::{ray::Ray, Matrix4Ext},
        pool::Handle,
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    material::{shader::SamplerFallback, PropertyValue},
    rand::seq::SliceRandom,
    resource::model::Model,
    scene::{
        collider::{BitMask, InteractionGroups},
        graph::{
            physics::{FeatureId, Intersection, RayCastOptions},
            Graph,
        },
        node::{Node, TypeUuidProvider},
        Scene,
    },
    script::{
        ScriptContext, ScriptDeinitContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait,
    },
    utils::{self, log::Log},
};
use std::hash::{Hash, Hasher};

pub mod definition;
pub mod projectile;
pub mod sight;

pub struct WeaponMessage {
    pub weapon: Handle<Node>,
    pub data: WeaponMessageData,
}

pub enum WeaponMessageData {
    Shoot { direction: Option<Vector3<f32>> },
    Removed,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Weapon {
    kind: WeaponKind,
    shot_point: Handle<Node>,
    muzzle_flash: Handle<Node>,
    shot_light: Handle<Node>,
    shot_position: Vector3<f32>,
    muzzle_flash_timer: f32,
    flash_light: Handle<Node>,
    flash_light_enabled: bool,

    #[visit(optional)]
    projectile: Option<Model>,

    #[visit(optional)]
    laser_sight: Handle<Node>,

    #[reflect(hidden)]
    owner: Handle<Node>,

    #[reflect(hidden)]
    #[visit(optional)]
    last_shot_time: f32,

    #[reflect(hidden)]
    #[visit(skip)]
    pub definition: &'static WeaponDefinition,

    #[reflect(hidden)]
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
            projectile: None,
            laser_sight: Default::default(),
            self_handle: Default::default(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub hit_actor: Handle<Node>, // Can be None if level geometry was hit.
    pub shooter_actor: Handle<Node>,
    pub position: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub collider: Handle<Node>,
    pub feature: FeatureId,
    pub hit_box: Option<HitBox>,
    pub query_buffer: Vec<Intersection>,
}

impl PartialEq for Hit {
    fn eq(&self, other: &Self) -> bool {
        self.hit_actor == other.hit_actor
            && self.shooter_actor == other.shooter_actor
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

impl Weapon {
    /// Checks intersection of given ray with actors and environment.
    pub fn ray_hit(
        begin: Vector3<f32>,
        end: Vector3<f32>,
        shooter: Handle<Node>,
        actors: &[Handle<Node>],
        graph: &mut Graph,
        ignored_collider: Handle<Node>,
    ) -> Option<Hit> {
        if begin == end {
            return None;
        }

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

                        // Ignore intersections with owners.
                        if shooter == actor_handle {
                            continue 'actor_loop;
                        }

                        return Some(Hit {
                            hit_actor: actor_handle,
                            shooter_actor: shooter,
                            position: hit.position.coords,
                            normal: hit.normal,
                            collider: hit.collider,
                            feature: hit.feature,
                            hit_box: Some(*hit_box),
                            query_buffer,
                        });
                    }
                }

                // If none of hit boxes is hit, then check if we hit actor's capsule.
                if character.capsule_collider == hit.collider {
                    return Some(Hit {
                        hit_actor: actor_handle,
                        shooter_actor: shooter,
                        position: hit.position.coords,
                        normal: hit.normal,
                        collider: hit.collider,
                        feature: hit.feature,
                        hit_box: None,
                        query_buffer,
                    });
                }
            }

            if is_hitbox_hit {
                None
            } else {
                Some(Hit {
                    hit_actor: Handle::NONE,
                    shooter_actor: shooter,
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

    pub fn laser_sight(&mut self) -> Handle<Node> {
        self.laser_sight
    }

    pub fn can_shoot(&self, elapsed_time: f32) -> bool {
        elapsed_time - self.last_shot_time >= self.definition.shoot_interval
    }

    fn shoot(
        &mut self,
        self_handle: Handle<Node>,
        scene: &mut Scene,
        elapsed_time: f32,
        resource_manager: &ResourceManager,
        direction: Option<Vector3<f32>>,
        sound_manager: &SoundManager,
    ) {
        self.last_shot_time = elapsed_time;

        let position = self.shot_position(&scene.graph);

        if let Some(random_shot_sound) = self
            .definition
            .shot_sounds
            .choose(&mut fyrox::rand::thread_rng())
        {
            sound_manager.play_sound(&mut scene.graph, random_shot_sound, position, 1.0, 5.0, 3.0);
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

        let shot_position = self.shot_position(&scene.graph);
        let direction = direction
            .unwrap_or_else(|| self.shot_direction(&scene.graph))
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        if let Some(model) = self.projectile.as_ref() {
            Projectile::spawn(
                model,
                scene,
                direction,
                shot_position,
                self_handle,
                Default::default(),
            );
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
    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.definition = Self::definition(self.kind);
        self.self_handle = ctx.handle;

        ctx.message_dispatcher
            .subscribe_to::<WeaponMessage>(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        ctx.message_sender.send_global(WeaponMessage {
            weapon: ctx.node_handle,
            data: WeaponMessageData::Removed,
        });
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let node = &mut ctx.scene.graph[ctx.handle];
        self.shot_position = node.global_position();

        self.muzzle_flash_timer -= ctx.dt;
        if self.muzzle_flash_timer <= 0.0 && self.muzzle_flash.is_some() {
            ctx.scene.graph[self.muzzle_flash].set_visibility(false);
            ctx.scene.graph[self.shot_light].set_visibility(false);
        }

        if let Some(flash_light) = ctx.scene.graph.try_get_mut(self.flash_light) {
            flash_light.set_visibility(self.flash_light_enabled);
        }
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        if let Some(msg) = message.downcast_ref::<WeaponMessage>() {
            if msg.weapon != ctx.handle {
                return;
            }

            if let WeaponMessageData::Shoot { direction } = msg.data {
                let level = current_level_ref(ctx.plugins).unwrap();

                self.shoot(
                    ctx.handle,
                    ctx.scene,
                    ctx.elapsed_time,
                    ctx.resource_manager,
                    direction,
                    &level.sound_manager,
                );
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

pub fn try_weapon_mut(handle: Handle<Node>, graph: &mut Graph) -> Option<&mut Weapon> {
    graph
        .try_get_mut(handle)
        .and_then(|node| node.try_get_script_mut::<Weapon>())
}

pub fn weapon_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Weapon {
    graph[handle].try_get_script_mut::<Weapon>().unwrap()
}

pub fn try_weapon_ref(handle: Handle<Node>, graph: &Graph) -> Option<&Weapon> {
    graph
        .try_get(handle)
        .and_then(|node| node.try_get_script::<Weapon>())
}

pub fn weapon_ref(handle: Handle<Node>, graph: &Graph) -> &Weapon {
    try_weapon_ref(handle, graph).unwrap()
}
