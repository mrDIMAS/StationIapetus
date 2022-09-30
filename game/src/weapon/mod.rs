//! Weapon related stuff.

use crate::{
    bot::{try_get_bot_mut, BotCommand},
    character::{
        character_mut, character_ref, try_get_character_mut, try_get_character_ref, Character,
        CharacterCommand, HitBox,
    },
    current_level_mut, current_level_ref, effects,
    effects::EffectKind,
    game_ref,
    level::trail::ShotTrail,
    message::Message,
    sound::SoundKind,
    weapon::{
        definition::{ShotEffect, WeaponDefinition, WeaponKind, WeaponProjectile},
        projectile::{Damage, Projectile},
        sight::{LaserSight, SightReaction},
    },
    CollisionGroups, Decal, MessageSender,
};
use fyrox::{
    core::{
        algebra::{Matrix3, Point3, UnitQuaternion, Vector3},
        color::Color,
        inspect::prelude::*,
        math::{ray::Ray, vector_to_quat, Matrix4Ext},
        parking_lot::Mutex,
        pool::Handle,
        reflect::Reflect,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    material::{shader::SamplerFallback, Material, PropertyValue},
    rand::seq::SliceRandom,
    scene::{
        base::BaseBuilder,
        collider::{BitMask, ColliderShape, InteractionGroups},
        graph::{
            physics::{FeatureId, Intersection, RayCastOptions},
            Graph,
        },
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder, RenderPath,
        },
        node::{Node, TypeUuidProvider},
        rigidbody::RigidBody,
        transform::TransformBuilder,
        Scene,
    },
    script::{Script, ScriptContext, ScriptDeinitContext, ScriptTrait},
    utils::{self, log::Log},
};
use std::{
    hash::{Hash, Hasher},
    path::PathBuf,
    sync::Arc,
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

#[derive(Clone, Debug)]
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

                        let who = shooter;

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

    pub fn shoot_ray(
        graph: &mut Graph,
        resource_manager: &ResourceManager,
        actors: &[Handle<Node>],
        shooter: Handle<Node>,
        begin: Vector3<f32>,
        end: Vector3<f32>,
        damage: Damage,
        shot_effect: ShotEffect,
        sender: &MessageSender,
        critical_shot_probability: f32,
    ) -> Option<Hit> {
        // Do immediate intersection test and solve it.
        let (trail_len, hit_point, hit) = if let Some(hit) =
            Weapon::ray_hit(begin, end, shooter, actors, graph, Default::default())
        {
            effects::create(
                if hit.actor.is_some() {
                    EffectKind::BloodSpray
                } else {
                    EffectKind::BulletImpact
                },
                graph,
                resource_manager,
                hit.position,
                vector_to_quat(hit.normal),
            );

            // Just send new messages, instead of doing everything manually here.
            sender.send(Message::PlayEnvironmentSound {
                collider: hit.collider,
                feature: hit.feature,
                position: hit.position,
                sound_kind: SoundKind::Impact,
                gain: 1.0,
                rolloff_factor: 1.0,
                radius: 0.5,
            });

            if let Some(character) = try_get_character_mut(hit.actor, graph) {
                character.push_command(CharacterCommand::Damage {
                    who: hit.who,
                    hitbox: hit.hit_box,
                    amount: damage
                        .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor))
                        .amount(),
                    critical_shot_probability,
                });
            }

            let dir = hit.position - begin;

            let hit_collider_body = graph[hit.collider].parent();
            let parent =
                if let Some(collider_parent) = graph[hit_collider_body].cast_mut::<RigidBody>() {
                    collider_parent.apply_force_at_point(
                        dir.try_normalize(std::f32::EPSILON)
                            .unwrap_or_default()
                            .scale(30.0),
                        hit.position,
                    );
                    hit_collider_body
                } else {
                    Default::default()
                };

            if let Some(hitbox) = hit.hit_box {
                try_get_bot_mut(hit.actor, graph)
                    .unwrap()
                    .commands_queue
                    .push_back(BotCommand::HandleImpact {
                        handle: hitbox.bone,
                        impact_point: hit.position,
                        direction: dir,
                    });
            }

            Decal::new_bullet_hole(
                resource_manager,
                graph,
                hit.position,
                hit.normal,
                parent,
                if hit.actor.is_some() {
                    Color::opaque(160, 0, 0)
                } else {
                    Color::opaque(20, 20, 20)
                },
            );

            // Add blood splatter on a surface behind an actor that was shot.
            if !try_get_character_ref(hit.actor, graph).map_or(true, |a| a.is_dead()) {
                for intersection in hit.query_buffer.iter() {
                    if matches!(
                        graph[intersection.collider].as_collider().shape(),
                        ColliderShape::Trimesh(_)
                    ) && intersection.position.coords.metric_distance(&hit.position) < 2.0
                    {
                        Decal::add_to_graph(
                            graph,
                            intersection.position.coords,
                            dir,
                            Handle::NONE,
                            Color::opaque(255, 255, 255),
                            Vector3::new(0.45, 0.45, 0.2),
                            resource_manager.request_texture(
                                "data/textures/decals/BloodSplatter_BaseColor.png",
                            ),
                        );

                        break;
                    }
                }
            }

            (dir.norm(), hit.position, Some(hit))
        } else {
            (30.0, end, None)
        };

        match shot_effect {
            ShotEffect::Smoke => {
                let effect = effects::create(
                    EffectKind::Smoke,
                    graph,
                    resource_manager,
                    begin,
                    Default::default(),
                );
                graph[effect].set_script(Some(Script::new(ShotTrail::new(5.0))));
            }
            ShotEffect::Beam => {
                let trail_radius = 0.0014;
                MeshBuilder::new(
                    BaseBuilder::new()
                        .with_script(Script::new(ShotTrail::new(0.2)))
                        .with_cast_shadows(false)
                        .with_local_transform(
                            TransformBuilder::new()
                                .with_local_position(begin)
                                .with_local_scale(Vector3::new(
                                    trail_radius,
                                    trail_radius,
                                    trail_len,
                                ))
                                .with_local_rotation(UnitQuaternion::face_towards(
                                    &(end - begin),
                                    &Vector3::y(),
                                ))
                                .build(),
                        ),
                )
                .with_surfaces(vec![SurfaceBuilder::new(Arc::new(Mutex::new(
                    SurfaceData::make_cylinder(
                        6,
                        1.0,
                        1.0,
                        false,
                        &UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 90.0f32.to_radians())
                            .to_homogeneous(),
                    ),
                )))
                .with_material(Arc::new(Mutex::new({
                    let mut material = Material::standard();
                    Log::verify(material.set_property(
                        &ImmutableString::new("diffuseColor"),
                        PropertyValue::Color(Color::from_rgba(255, 255, 255, 120)),
                    ));
                    material
                })))
                .build()])
                .with_render_path(RenderPath::Forward)
                .build(graph);
            }
            ShotEffect::Rail => {
                let effect = effects::create_rail(
                    graph,
                    resource_manager,
                    begin,
                    hit_point,
                    Color::opaque(255, 0, 0),
                );
                graph[effect].set_script(Some(Script::new(ShotTrail::new(5.0))));
            }
        }

        hit
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
        resource_manager: &ResourceManager,
        direction: Option<Vector3<f32>>,
        sender: &MessageSender,
        actors: &[Handle<Node>],
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
            WeaponProjectile::Projectile(projectile) => {
                Projectile::add_to_scene(
                    projectile,
                    &resource_manager,
                    scene,
                    direction,
                    position,
                    self_handle,
                    Default::default(),
                );
            }
            WeaponProjectile::Ray { damage } => {
                if let Some(hit) = Self::shoot_ray(
                    &mut scene.graph,
                    resource_manager,
                    actors,
                    self_handle,
                    position,
                    position + direction.scale(1000.0),
                    damage,
                    self.definition.shot_effect,
                    sender,
                    self.definition.base_critical_shot_probability,
                ) {
                    if hit.actor.is_some() {
                        self.set_sight_reaction(SightReaction::HitDetected);
                    }
                }
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
        self.laser_sight = LaserSight::new(ctx.scene, ctx.resource_manager.clone());
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.definition = Self::definition(self.kind);
        self.self_handle = ctx.handle;
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        if let Some(level) = current_level_mut(ctx.plugins) {
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
        let level = current_level_ref(ctx.plugins).unwrap();

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
                ctx.resource_manager,
                request.direction,
                &game.message_sender,
                &level.actors,
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
