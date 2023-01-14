//! Weapon related stuff.

use crate::{
    character::{
        character_mut, character_ref, try_get_character_ref, CharacterMessage,
        CharacterMessageData, DamageDealer, HitBox,
    },
    current_level_mut, current_level_ref, effects,
    effects::EffectKind,
    level::trail::ShotTrail,
    sound::{SoundKind, SoundManager},
    weapon::{
        definition::{ShotEffect, WeaponDefinition, WeaponKind},
        projectile::{Damage, Projectile},
    },
    CollisionGroups, Decal,
};
use fyrox::resource::model::Model;
use fyrox::{
    core::{
        algebra::{Matrix3, Point3, UnitQuaternion, Vector3},
        color::Color,
        math::{ray::Ray, vector_to_quat, Matrix4Ext},
        pool::Handle,
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    material::{shader::SamplerFallback, Material, PropertyValue, SharedMaterial},
    rand::seq::SliceRandom,
    scene::{
        base::BaseBuilder,
        collider::{BitMask, ColliderShape, InteractionGroups},
        graph::{
            physics::{FeatureId, Intersection, RayCastOptions},
            Graph,
        },
        mesh::{
            surface::{SurfaceBuilder, SurfaceData, SurfaceSharedData},
            MeshBuilder, RenderPath,
        },
        node::{Node, TypeUuidProvider},
        rigidbody::{RigidBody, RigidBodyType},
        transform::TransformBuilder,
        Scene,
    },
    script::{Script, ScriptContext, ScriptDeinitContext, ScriptMessageSender, ScriptTrait},
    utils::{self, log::Log},
};
use std::hash::{Hash, Hasher};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

pub mod definition;
pub mod projectile;
pub mod sight;

#[derive(Debug, Default, Clone)]
pub struct ShotRequest {
    direction: Option<Vector3<f32>>,
}

#[derive(Clone, Debug, Visit, Reflect, AsRefStr, EnumString, EnumVariantNames)]
pub enum WeaponProjectile {
    Projectile(Option<Model>),
    /// For high-speed "projectiles".
    Ray {
        damage: Damage,
    },
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
    pub enabled: bool,

    #[visit(optional)]
    projectile: WeaponProjectile,

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
    shot_request: Option<ShotRequest>,

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
            enabled: true,
            projectile: WeaponProjectile::Ray {
                damage: Damage::Point(10.0),
            },
            laser_sight: Default::default(),
            shot_request: None,
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

    pub fn shoot_ray(
        graph: &mut Graph,
        resource_manager: &ResourceManager,
        actors: &[Handle<Node>],
        shooter: Handle<Node>,
        begin: Vector3<f32>,
        end: Vector3<f32>,
        damage: Damage,
        shot_effect: ShotEffect,
        sound_manager: &SoundManager,
        critical_shot_probability: f32,
        script_message_sender: &ScriptMessageSender,
    ) -> Option<Hit> {
        // Do immediate intersection test and solve it.
        let (trail_len, hit_point, hit) = if let Some(hit) =
            Weapon::ray_hit(begin, end, shooter, actors, graph, Default::default())
        {
            effects::create(
                if hit.hit_actor.is_some() {
                    EffectKind::BloodSpray
                } else {
                    EffectKind::BulletImpact
                },
                graph,
                resource_manager,
                hit.position,
                vector_to_quat(hit.normal),
            );

            sound_manager.play_environment_sound(
                graph,
                hit.collider,
                hit.feature,
                hit.position,
                SoundKind::Impact,
                1.0,
                1.0,
                0.5,
            );

            script_message_sender.send_global(CharacterMessage {
                character: hit.hit_actor,
                data: CharacterMessageData::Damage {
                    dealer: DamageDealer {
                        entity: hit.shooter_actor,
                    },
                    hitbox: hit.hit_box,
                    amount: damage
                        .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor))
                        .amount(),
                    critical_shot_probability,
                },
            });

            let dir = hit.position - begin;

            let mut node = hit.collider;
            while let Some(node_ref) = graph.try_get_mut(node) {
                if let Some(rigid_body) = node_ref.query_component_mut::<RigidBody>() {
                    if rigid_body.body_type() == RigidBodyType::Dynamic {
                        rigid_body.apply_force_at_point(
                            dir.try_normalize(f32::EPSILON)
                                .unwrap_or_default()
                                .scale(30.0),
                            hit.position,
                        );
                    }
                }
                node = node_ref.parent();
            }

            if let Some(hit_box) = hit.hit_box {
                script_message_sender.send_to_target(
                    hit.hit_actor,
                    CharacterMessage {
                        character: hit.hit_actor,
                        data: CharacterMessageData::HandleImpact {
                            handle: hit_box.bone,
                            impact_point: hit.position,
                            direction: dir,
                        },
                    },
                );
            }

            Decal::new_bullet_hole(
                resource_manager,
                graph,
                hit.position,
                hit.normal,
                hit.collider,
                if hit.hit_actor.is_some() {
                    Color::opaque(160, 0, 0)
                } else {
                    Color::opaque(20, 20, 20)
                },
            );

            // Add blood splatter on a surface behind an actor that was shot.
            if try_get_character_ref(hit.hit_actor, graph).is_some() {
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
                .with_surfaces(vec![SurfaceBuilder::new(SurfaceSharedData::new(
                    SurfaceData::make_cylinder(
                        6,
                        1.0,
                        1.0,
                        false,
                        &UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 90.0f32.to_radians())
                            .to_homogeneous(),
                    ),
                ))
                .with_material(SharedMaterial::new({
                    let mut material = Material::standard();
                    Log::verify(material.set_property(
                        &ImmutableString::new("diffuseColor"),
                        PropertyValue::Color(Color::from_rgba(255, 255, 255, 120)),
                    ));
                    material
                }))
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

    pub fn laser_sight(&mut self) -> Handle<Node> {
        self.laser_sight
    }

    pub fn can_shoot(&self, elapsed_time: f32) -> bool {
        elapsed_time - self.last_shot_time >= self.definition.shoot_interval
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
        sound_manager: &SoundManager,
        actors: &[Handle<Node>],
        script_message_sender: &ScriptMessageSender,
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

        let position = self.shot_position(&scene.graph);
        let direction = direction
            .unwrap_or_else(|| self.shot_direction(&scene.graph))
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        match self.projectile {
            WeaponProjectile::Projectile(ref projectile) => {
                if let Some(model) = projectile {
                    Projectile::add_to_scene(
                        model,
                        scene,
                        direction,
                        position,
                        self_handle,
                        Default::default(),
                    );
                }
            }
            WeaponProjectile::Ray { damage } => {
                Self::shoot_ray(
                    &mut scene.graph,
                    resource_manager,
                    actors,
                    self_handle,
                    position,
                    position + direction.scale(1000.0),
                    damage,
                    self.definition.shot_effect,
                    sound_manager,
                    self.definition.base_critical_shot_probability,
                    script_message_sender,
                );
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

        if let Some(flash_light) = ctx.scene.graph.try_get_mut(self.flash_light) {
            flash_light.set_visibility(self.flash_light_enabled);
        }

        if let Some(request) = self.shot_request.take() {
            self.shoot(
                ctx.handle,
                ctx.scene,
                ctx.elapsed_time,
                ctx.resource_manager,
                request.direction,
                &level.sound_manager,
                &level.actors,
                ctx.message_sender,
            );
        }
    }

    fn restore_resources(&mut self, resource_manager: ResourceManager) {
        if let WeaponProjectile::Projectile(ref mut model) = self.projectile {
            resource_manager
                .state()
                .containers_mut()
                .models
                .try_restore_optional_resource(model)
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
