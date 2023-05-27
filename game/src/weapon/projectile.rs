use crate::{
    character::{
        character_ref, try_get_character_ref, Character, CharacterMessage, CharacterMessageData,
        DamageDealer, DamagePosition, HitBox,
    },
    level::decal::Decal,
    utils::ResourceProxy,
    CollisionGroups, Game, Level, Weapon,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        color::Color,
        math::{ray::Ray, vector_to_quat, Vector3Ext},
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    rand::seq::SliceRandom,
    resource::{
        model::{ModelResource, ModelResourceExtension},
        texture::Texture,
    },
    scene::{
        collider::{BitMask, Collider, ColliderShape, InteractionGroups},
        graph::{
            physics::{FeatureId, Intersection, RayCastOptions},
            Graph,
        },
        node::Node,
        rigidbody::RigidBody,
        sound::SoundBufferResource,
        Scene,
    },
    script::{ScriptContext, ScriptTrait},
};
use serde::Deserialize;
use std::hash::{Hash, Hasher};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(
    Deserialize, Copy, Clone, Debug, Visit, Reflect, AsRefStr, EnumString, EnumVariantNames,
)]
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
        fyrox::utils::hash_as_bytes(self, state);
    }
}

impl Eq for Hit {}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Projectile {
    #[reflect(hidden)]
    dir: Vector3<f32>,

    pub owner: Handle<Node>,

    #[reflect(hidden)]
    initial_velocity: Vector3<f32>,

    #[reflect(hidden)]
    last_position: Vector3<f32>,

    #[visit(optional)]
    use_ray_casting: bool,

    #[visit(optional)]
    speed: Option<f32>,

    #[visit(optional, rename = "ImpactEffect")]
    environment_impact_effect: Option<ModelResource>,

    #[visit(optional)]
    flesh_impact_effect: Option<ModelResource>,

    #[visit(optional)]
    impact_sound: Option<SoundBufferResource>,

    #[visit(optional)]
    #[reflect(
        description = "A prefab that will be instantiated when the projectile is just appeared (spawned)."
    )]
    appear_effect: Option<ModelResource>,

    #[visit(optional)]
    #[reflect(
        description = "Random prefab will be instantiated from the list when the projectile is just appeared (spawned)."
    )]
    random_appear_effects: Vec<ResourceProxy<ModelResource>>,

    #[visit(optional)]
    #[reflect(
        description = "Limit lifetime of the projectile just one update frame. Useful for ray-based projectiles."
    )]
    one_frame: bool,

    #[visit(optional)]
    damage: Damage,

    #[visit(optional)]
    #[reflect(min_value = 0.0, max_value = 1.0)]
    critical_hit_probability: f32,

    // A handle to collider of the projectile. It is used as a cache to prevent searching for it
    // every frame.
    #[visit(skip)]
    #[reflect(hidden)]
    collider: Handle<Node>,
}

impl_component_provider!(Projectile);

impl TypeUuidProvider for Projectile {
    fn type_uuid() -> Uuid {
        uuid!("6b60c75e-83cf-406b-8106-e87d5ab98132")
    }
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
            impact_sound: None,
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
        let instance_handle = resource.instantiate(scene);

        let instance_ref = &mut scene.graph[instance_handle];

        instance_ref
            .local_transform_mut()
            .set_position(position)
            .set_rotation(vector_to_quat(dir));

        if let Some(projectile) = instance_ref.try_get_script_mut::<Projectile>() {
            projectile.initial_velocity = initial_velocity;
            projectile.dir = dir.try_normalize(f32::EPSILON).unwrap_or_else(Vector3::y);
            projectile.owner = owner;
        }

        scene
            .graph
            .update_hierarchical_data_for_descendants(instance_handle);

        instance_handle
    }
}

fn ray_hit(
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
        // Check if there was an intersection with an actor.
        'actor_loop: for &actor_handle in actors.iter() {
            let character = character_ref(actor_handle, graph);

            // Check hit boxes first.
            for hit_box in character.hit_boxes.iter() {
                if hit_box.collider == hit.collider {
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
            .and_then(|vfx| vfx.0.as_ref())
        {
            vfx.instantiate_at(ctx.scene, current_position, vector_to_quat(self.dir));
        }
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.collider = ctx
            .scene
            .graph
            .find(ctx.handle, &mut |n| {
                n.query_component_ref::<Collider>().is_some()
            })
            .map(|(h, _)| h)
            .unwrap_or_default();
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = Game::game_ref(ctx.plugins);
        let level = Level::try_get(ctx.plugins).unwrap();

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
                &Level::try_get(ctx.plugins).unwrap().actors,
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
                            } else if owner_node
                                .script()
                                .map(|s| s.query_component_ref::<Character>())
                                .is_some()
                            {
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
                            for &actor_handle in level.actors.iter() {
                                let character = character_ref(actor_handle, &ctx.scene.graph);
                                for hit_box in character.hit_boxes.iter() {
                                    if hit_box.collider == other_collider {
                                        hit = Some(Hit {
                                            hit_actor: actor_handle,
                                            shooter_actor: owner_character,
                                            position: position
                                                + if self.collider == contact.collider1 {
                                                    point.local_p2
                                                } else {
                                                    point.local_p1
                                                },
                                            normal: manifold.normal,
                                            collider: other_collider,
                                            feature: FeatureId::Unknown,
                                            hit_box: Some(*hit_box),
                                            query_buffer: vec![],
                                        });

                                        break 'contact_loop;
                                    }
                                }
                            }

                            // Also, handle contacts with environment.
                            hit = Some(Hit {
                                hit_actor: Default::default(),
                                shooter_actor: owner_character,
                                position: position
                                    + if self.collider == contact.collider1 {
                                        point.local_p2
                                    } else {
                                        point.local_p1
                                    },
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

        if let Some(hit) = hit {
            let damage = self
                .damage
                .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor));

            match damage {
                Damage::Splash { radius, amount } => {
                    let level = Level::try_get(ctx.plugins).unwrap();
                    // Just find out actors which must be damaged and re-cast damage message for each.
                    for &actor_handle in level.actors.iter() {
                        let character = character_ref(actor_handle, &ctx.scene.graph);
                        // TODO: Add occlusion test. This will hit actors through walls.
                        let character_position = character.position(&ctx.scene.graph);
                        if character_position.metric_distance(&position) <= radius {
                            ctx.message_sender.send_global(CharacterMessage {
                                character: actor_handle,
                                data: CharacterMessageData::Damage {
                                    dealer: DamageDealer {
                                        entity: hit.shooter_actor,
                                    },
                                    hitbox: None,
                                    /// TODO: Maybe collect all hitboxes?
                                    amount,
                                    critical_hit_probability: self.critical_hit_probability,
                                    position: Some(DamagePosition {
                                        point: hit.position,
                                        direction,
                                    }),
                                },
                            });
                        }
                    }
                }
                Damage::Point(amount) => {
                    ctx.message_sender.send_global(CharacterMessage {
                        character: hit.hit_actor,
                        data: CharacterMessageData::Damage {
                            dealer: DamageDealer {
                                entity: hit.shooter_actor,
                            },
                            hitbox: hit.hit_box,
                            amount,
                            critical_hit_probability: self.critical_hit_probability,
                            position: Some(DamagePosition {
                                point: hit.position,
                                direction,
                            }),
                        },
                    });
                }
            }

            if let Some(effect_prefab) = if hit.hit_actor.is_some() {
                self.flesh_impact_effect.as_ref()
            } else {
                self.environment_impact_effect.as_ref()
            } {
                effect_prefab.instantiate_at(ctx.scene, hit.position, vector_to_quat(hit.normal));
            }

            if let Some(impact_sound) = self.impact_sound.as_ref() {
                game.level
                    .as_ref()
                    .unwrap()
                    .sound_manager
                    .play_sound_buffer(
                        &mut ctx.scene.graph,
                        impact_sound,
                        hit.position,
                        1.0,
                        4.0,
                        3.0,
                    );
            }

            Decal::new_bullet_hole(
                ctx.resource_manager,
                &mut ctx.scene.graph,
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
            if try_get_character_ref(hit.hit_actor, &ctx.scene.graph).is_some() {
                for intersection in hit.query_buffer.iter() {
                    if matches!(
                        ctx.scene.graph[intersection.collider].as_collider().shape(),
                        ColliderShape::Trimesh(_)
                    ) && intersection.position.coords.metric_distance(&hit.position) < 2.0
                    {
                        Decal::spawn(
                            &mut ctx.scene.graph,
                            intersection.position.coords,
                            hit.normal,
                            Handle::NONE,
                            Color::opaque(255, 255, 255),
                            Vector3::new(0.45, 0.45, 0.2),
                            ctx.resource_manager.request::<Texture, _>(
                                "data/textures/decals/BloodSplatter_BaseColor.png",
                            ),
                        );

                        break;
                    }
                }
            }

            // Defer destruction.
            ctx.scene.graph[ctx.handle].set_lifetime(Some(0.0));
        }

        if self.one_frame {
            ctx.scene.graph[ctx.handle].set_lifetime(Some(0.0));
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
