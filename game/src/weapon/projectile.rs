use crate::{
    character::{
        character_ref, try_get_character_ref, Character, CharacterMessage, CharacterMessageData,
        DamageDealer,
    },
    current_level_ref, game_ref,
    level::decal::Decal,
    weapon::Hit,
    CollisionGroups, Turret, Weapon,
};
use fyrox::{
    core::{
        algebra::Point3,
        algebra::Vector3,
        color::Color,
        futures::executor::block_on,
        math::{ray::Ray, vector_to_quat, Vector3Ext},
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    impl_component_provider,
    resource::model::Model,
    scene::{
        collider::{BitMask, Collider, ColliderShape, InteractionGroups},
        graph::{physics::FeatureId, physics::RayCastOptions, Graph},
        node::{Node, TypeUuidProvider},
        rigidbody::RigidBody,
        sound::SoundBufferResource,
        Scene,
    },
    script::{ScriptContext, ScriptTrait},
};
use serde::Deserialize;
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

    #[visit(optional)]
    impact_effect: Option<Model>,

    #[visit(optional)]
    impact_sound: Option<SoundBufferResource>,

    #[visit(optional)]
    #[reflect(
        description = "A prefab that will be instantiated when the projectile is just appeared (spawned)."
    )]
    appear_effect: Option<Model>,

    #[visit(optional)]
    #[reflect(
        description = "Limit lifetime of the projectile just one update frame. Useful for ray-based projectiles."
    )]
    one_frame: bool,

    #[visit(optional)]
    damage: Damage,

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
            speed: Some(10.0),
            impact_effect: None,
            impact_sound: None,
            appear_effect: None,
            one_frame: false,
            damage: Default::default(),
            collider: Default::default(),
        }
    }
}

impl Projectile {
    pub fn spawn(
        resource: &Model,
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
    fn on_init(&mut self, context: &mut ScriptContext) {
        let node = &mut context.scene.graph[context.handle];

        let current_position = node.global_position();

        self.last_position = current_position;

        if let Some(rigid_body) = node.cast_mut::<RigidBody>() {
            rigid_body.set_lin_vel(self.initial_velocity);
        }

        if let Some(appear_effect) = self.appear_effect.as_ref() {
            let root = appear_effect.instantiate(context.scene);

            context.scene.graph[root]
                .local_transform_mut()
                .set_position(current_position)
                .set_rotation(vector_to_quat(self.dir));

            context
                .scene
                .graph
                .update_hierarchical_data_for_descendants(root);
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
        let game = game_ref(ctx.plugins);
        let level = current_level_ref(ctx.plugins).unwrap();

        // Movement of kinematic projectiles is controlled explicitly.
        if let Some(speed) = self.speed {
            let total_velocity = self.dir.scale(speed);
            ctx.scene.graph[ctx.handle]
                .local_transform_mut()
                .offset(total_velocity);

            ctx.scene
                .graph
                .update_hierarchical_data_for_descendants(ctx.handle);
        }

        // Reduce initial velocity down to zero over time. This is needed because projectile
        // stabilizes its movement over time.
        self.initial_velocity.follow(&Vector3::default(), 0.15);

        let position = ctx.scene.graph[ctx.handle].global_position();

        let mut hit = None;

        if self.use_ray_casting {
            hit = ray_hit(
                self.last_position,
                position,
                self.owner,
                &current_level_ref(ctx.plugins).unwrap().actors,
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
                        }
                    }
                }
            }
        }

        if let Some(hit) = hit {
            let damage = self
                .damage
                .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor));

            let critical_shot_probability =
                ctx.scene
                    .graph
                    .try_get(self.owner)
                    .map_or(0.0, |owner_node| {
                        if let Some(weapon) = owner_node.try_get_script::<Weapon>() {
                            weapon.definition.base_critical_shot_probability
                        } else if owner_node.has_script::<Turret>() {
                            0.01
                        } else {
                            0.0
                        }
                    });

            match damage {
                Damage::Splash { radius, amount } => {
                    let level = current_level_ref(ctx.plugins).unwrap();
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
                                    critical_shot_probability,
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
                            critical_shot_probability,
                        },
                    });
                }
            }

            if let Ok(effect_prefab) = block_on(ctx.resource_manager.request_model(
                if hit.hit_actor.is_some() {
                    "data/models/blood_splatter.rgs"
                } else {
                    "data/models/bullet_impact.rgs"
                },
            )) {
                let instance = effect_prefab.instantiate(ctx.scene);
                ctx.scene.graph[instance]
                    .local_transform_mut()
                    .set_position(hit.position)
                    .set_rotation(vector_to_quat(hit.normal));
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
            if try_get_character_ref(hit.hit_actor, &mut ctx.scene.graph).is_some() {
                for intersection in hit.query_buffer.iter() {
                    if matches!(
                        ctx.scene.graph[intersection.collider].as_collider().shape(),
                        ColliderShape::Trimesh(_)
                    ) && intersection.position.coords.metric_distance(&hit.position) < 2.0
                    {
                        Decal::add_to_graph(
                            &mut ctx.scene.graph,
                            intersection.position.coords,
                            -hit.normal,
                            Handle::NONE,
                            Color::opaque(255, 255, 255),
                            Vector3::new(0.45, 0.45, 0.2),
                            ctx.resource_manager.request_texture(
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
