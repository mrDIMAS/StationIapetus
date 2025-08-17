use crate::level::hit_box::HitBoxDamage;
use crate::{
    character::{Character, DamageDealer, DamagePosition},
    level::{
        decal::Decal,
        hit_box::{HitBox, HitBoxMessage},
    },
    CollisionGroups, Game, Weapon,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        color::Color,
        math::{ray::Ray, vector_to_quat, Vector3Ext},
        pool::Handle,
        reflect::prelude::*,
        stub_uuid_provider,
        type_traits::prelude::*,
        visitor::prelude::*,
    },
    graph::{BaseSceneGraph, SceneGraph, SceneGraphNode},
    rand::seq::SliceRandom,
    resource::model::{ModelResource, ModelResourceExtension},
    scene::{
        collider::{BitMask, Collider, InteractionGroups},
        graph::{
            physics::{FeatureId, Intersection, RayCastOptions},
            Graph,
        },
        node::Node,
        rigidbody::RigidBody,
        Scene,
    },
    script::{RoutingStrategy, ScriptContext, ScriptTrait},
};
use serde::Deserialize;
use std::hash::{Hash, Hasher};
use strum_macros::{AsRefStr, EnumString, VariantNames};

#[derive(Deserialize, Copy, Clone, Debug, Visit, Reflect, AsRefStr, EnumString, VariantNames)]
pub enum Damage {
    Splash { radius: f32, amount: f32 },
    Point(f32),
}

stub_uuid_provider!(Damage);

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

#[derive(Clone, Debug)]
pub struct Hit {
    pub shooter_actor: Handle<Node>,
    pub position: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub collider: Handle<Node>,
    pub feature: FeatureId,
    pub hit_box: Option<Handle<Node>>,
    pub query_buffer: Vec<Intersection>,
}

impl PartialEq for Hit {
    fn eq(&self, other: &Self) -> bool {
        self.shooter_actor == other.shooter_actor
            && self.position == other.position
            && self.normal == other.normal
            && self.collider == other.collider
            && self.feature == other.feature
            && self.hit_box == other.hit_box
    }
}

impl Hash for Hit {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let bytes = unsafe {
            std::slice::from_raw_parts(self as *const Self as *const u8, size_of::<Self>())
        };
        bytes.hash(state)
    }
}

impl Eq for Hit {}

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "6b60c75e-83cf-406b-8106-e87d5ab98132")]
#[visit(optional)]
pub struct Projectile {
    #[reflect(hidden)]
    dir: Vector3<f32>,

    pub owner: Handle<Node>,

    #[reflect(hidden)]
    initial_velocity: Vector3<f32>,

    #[reflect(hidden)]
    last_position: Vector3<f32>,

    use_ray_casting: bool,

    speed: Option<f32>,

    #[visit(rename = "ImpactEffect")]
    environment_impact_effect: Option<ModelResource>,

    flesh_impact_effect: Option<ModelResource>,

    /// A prefab that will be instantiated when the projectile is just appeared (spawned).
    appear_effect: Option<ModelResource>,

    /// Random prefab will be instantiated from the list when the projectile is just appeared (spawned).
    random_appear_effects: Vec<Option<ModelResource>>,

    /// Limit lifetime of the projectile just one update frame. Useful for ray-based projectiles.
    one_frame: bool,

    damage: Damage,

    #[reflect(min_value = 0.0, max_value = 1.0)]
    critical_hit_probability: f32,

    // A handle to collider of the projectile. It is used as a cache to prevent searching for it
    // every frame.
    #[visit(skip)]
    #[reflect(hidden)]
    collider: Handle<Node>,
}

impl Default for Projectile {
    fn default() -> Self {
        Self {
            dir: Default::default(),
            owner: Default::default(),
            initial_velocity: Default::default(),
            last_position: Default::default(),
            use_ray_casting: true,
            speed: Some(1.0),
            environment_impact_effect: None,
            flesh_impact_effect: None,
            appear_effect: None,
            random_appear_effects: Default::default(),
            one_frame: false,
            damage: Default::default(),
            critical_hit_probability: 0.025,
            collider: Default::default(),
        }
    }
}

impl Projectile {
    pub fn spawn(
        resource: &ModelResource,
        scene: &mut Scene,
        dir: Vector3<f32>,
        position: Vector3<f32>,
        owner: Handle<Node>,
        initial_velocity: Vector3<f32>,
    ) -> Handle<Node> {
        let instance_handle = resource.instantiate_at(scene, position, vector_to_quat(dir));

        if let Some(projectile) = scene.graph[instance_handle].try_get_script_mut::<Projectile>() {
            projectile.initial_velocity = initial_velocity;
            projectile.dir = dir.try_normalize(f32::EPSILON).unwrap_or_else(Vector3::y);
            projectile.owner = owner;
        }

        instance_handle
    }
}

fn ray_hit(
    begin: Vector3<f32>,
    end: Vector3<f32>,
    shooter: Handle<Node>,
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
        if graph.try_get_script_of::<HitBox>(hit.collider).is_some() {
            return Some(Hit {
                shooter_actor: shooter,
                position: hit.position.coords,
                normal: hit.normal,
                collider: hit.collider,
                feature: hit.feature,
                hit_box: Some(hit.collider),
                query_buffer,
            });
        }

        Some(Hit {
            shooter_actor: shooter,
            position: hit.position.coords,
            normal: hit.normal,
            collider: hit.collider,
            feature: hit.feature,
            hit_box: None,
            query_buffer,
        })
    } else {
        None
    }
}

impl ScriptTrait for Projectile {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        let node = &mut ctx.scene.graph[ctx.handle];

        let current_position = node.global_position();

        self.last_position = current_position;

        if let Some(rigid_body) = node.cast_mut::<RigidBody>() {
            rigid_body.set_lin_vel(self.initial_velocity);
        }

        if let Some(appear_effect) = self.appear_effect.as_ref() {
            appear_effect.instantiate_at(ctx.scene, current_position, vector_to_quat(self.dir));
        }

        if let Some(vfx) = self
            .random_appear_effects
            .choose(&mut fyrox::rand::thread_rng())
            .and_then(|vfx| vfx.as_ref())
        {
            vfx.instantiate_at(ctx.scene, current_position, vector_to_quat(self.dir));
        }
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.collider = ctx
            .scene
            .graph
            .find(ctx.handle, &mut |n| n.component_ref::<Collider>().is_some())
            .map(|(h, _)| h)
            .unwrap_or_default();
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = ctx.plugins.get::<Game>();

        // Movement of kinematic projectiles is controlled explicitly.
        if let Some(speed) = self.speed {
            if speed != 0.0 {
                let total_velocity = self.dir.scale(speed);
                ctx.scene.graph[ctx.handle]
                    .local_transform_mut()
                    .offset(total_velocity);

                ctx.scene
                    .graph
                    .update_hierarchical_data_for_descendants(ctx.handle);
            }
        }

        // Reduce initial velocity down to zero over time. This is needed because projectile
        // stabilizes its movement over time.
        self.initial_velocity.follow(&Vector3::default(), 0.15);

        let position = ctx.scene.graph[ctx.handle].global_position();

        let direction = position - self.last_position;

        let mut hit = None;

        if self.use_ray_casting {
            hit = ray_hit(
                self.last_position,
                position,
                self.owner,
                &mut ctx.scene.graph,
                // Ignore self collider.
                self.collider,
            );
            self.last_position = position;
        }

        if hit.is_none() {
            // Collect hits from self collider.
            if let Some(collider) = ctx.scene.graph.try_get_of_type::<Collider>(self.collider) {
                let owner_character =
                    ctx.scene
                        .graph
                        .try_get(self.owner)
                        .map_or(Default::default(), |owner_node| {
                            if let Some(weapon) = owner_node.try_get_script::<Weapon>() {
                                weapon.owner
                            } else if owner_node.try_get_script_component::<Character>().is_some() {
                                self.owner
                            } else {
                                Default::default()
                            }
                        });

                'contact_loop: for contact in collider.contacts(&ctx.scene.graph.physics) {
                    let other_collider = if self.collider == contact.collider1 {
                        contact.collider2
                    } else {
                        contact.collider1
                    };
                    for manifold in contact.manifolds {
                        for point in manifold.points {
                            let contact_world_position = position
                                + if self.collider == contact.collider1 {
                                    point.local_p1
                                } else {
                                    point.local_p2
                                };

                            if ctx
                                .scene
                                .graph
                                .try_get_script_of::<HitBox>(other_collider)
                                .is_some()
                            {
                                hit = Some(Hit {
                                    shooter_actor: owner_character,
                                    position: contact_world_position,
                                    normal: manifold.normal,
                                    collider: other_collider,
                                    feature: FeatureId::Unknown,
                                    hit_box: Some(other_collider),
                                    query_buffer: vec![],
                                });

                                break 'contact_loop;
                            } else {
                                // Also, handle contacts with environment.
                                hit = Some(Hit {
                                    shooter_actor: owner_character,
                                    position: contact_world_position,
                                    normal: manifold.normal,
                                    collider: other_collider,
                                    feature: FeatureId::Unknown,
                                    hit_box: None,
                                    query_buffer: vec![],
                                });
                            }
                        }
                    }
                }
            }
        }

        if let Some(hit) = hit {
            match self.damage {
                Damage::Splash { radius, amount } => {
                    let level = game.level.as_ref().unwrap();

                    for &hit_box in level.hit_boxes.iter() {
                        let hit_box_ref = &ctx.scene.graph[hit_box];
                        if hit_box_ref.global_position().metric_distance(&position) <= radius {
                            ctx.message_sender.send_hierarchical(
                                hit_box,
                                RoutingStrategy::Up,
                                HitBoxMessage::Damage(HitBoxDamage {
                                    hit_box,
                                    damage: amount,
                                    dealer: DamageDealer {
                                        entity: hit.shooter_actor,
                                    },
                                    position: Some(DamagePosition {
                                        point: hit.position,
                                        direction,
                                    }),
                                    is_melee: false,
                                }),
                            );
                        }
                    }
                }
                Damage::Point(amount) => {
                    if let Some(hit_box) = hit.hit_box {
                        ctx.message_sender.send_hierarchical(
                            hit_box,
                            RoutingStrategy::Up,
                            HitBoxMessage::Damage(HitBoxDamage {
                                hit_box,
                                damage: amount,
                                dealer: DamageDealer {
                                    entity: hit.shooter_actor,
                                },
                                position: Some(DamagePosition {
                                    point: hit.position,
                                    direction,
                                }),
                                is_melee: false,
                            }),
                        );
                    }
                }
            }

            if hit.hit_box.is_none() {
                if let Some(effect_prefab) = self.environment_impact_effect.as_ref() {
                    effect_prefab.instantiate_at(
                        ctx.scene,
                        hit.position,
                        vector_to_quat(hit.normal),
                    );
                }

                Decal::new_bullet_hole(
                    ctx.resource_manager,
                    &mut ctx.scene.graph,
                    hit.position,
                    hit.normal,
                    hit.collider,
                    Color::opaque(20, 20, 20),
                );
            }

            if let Some(collider) = ctx.scene.graph.try_get(hit.collider) {
                if let Some(rigid_body) = ctx
                    .scene
                    .graph
                    .try_get_mut_of_type::<RigidBody>(collider.parent())
                {
                    rigid_body
                        .apply_force_at_point(direction.normalize().scale(50.0), hit.position);
                    rigid_body.wake_up();
                }
            }

            // Defer destruction.
            ctx.scene.graph[ctx.handle].set_lifetime(Some(0.0));
        }

        if self.one_frame {
            ctx.scene.graph[ctx.handle].set_lifetime(Some(0.0));
        }
    }
}
