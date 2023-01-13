use crate::character::Character;
use crate::{
    character::{character_ref, CharacterMessage, CharacterMessageData, DamageDealer},
    current_level_ref, effects,
    effects::EffectKind,
    game_ref,
    weapon::Hit,
    Turret, Weapon,
};
use fyrox::{
    core::{
        algebra::Vector3,
        futures::executor::block_on,
        math::{vector_to_quat, Vector3Ext},
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    impl_component_provider,
    lazy_static::lazy_static,
    scene::{
        collider::Collider,
        graph::physics::FeatureId,
        node::{Node, TypeUuidProvider},
        rigidbody::RigidBody,
        Scene,
    },
    script::{ScriptContext, ScriptTrait},
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Deserialize,
    Hash,
    Visit,
    Reflect,
    AsRefStr,
    EnumString,
    EnumVariantNames,
)]
pub enum ProjectileKind {
    Plasma,
    Grenade,
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

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Projectile {
    kind: ProjectileKind,

    #[reflect(hidden)]
    dir: Vector3<f32>,

    pub owner: Handle<Node>,

    #[reflect(hidden)]
    initial_velocity: Vector3<f32>,

    #[reflect(hidden)]
    last_position: Vector3<f32>,

    #[visit(optional)]
    use_ray_casting: bool,

    #[visit(skip)]
    #[reflect(hidden)]
    definition: &'static ProjectileDefinition,

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
            kind: ProjectileKind::Plasma,
            dir: Default::default(),
            owner: Default::default(),
            initial_velocity: Default::default(),
            last_position: Default::default(),
            use_ray_casting: true,
            definition: Self::get_definition(ProjectileKind::Plasma),
            collider: Default::default(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ProjectileDefinition {
    damage: Damage,
    speed: f32,
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

    pub fn add_to_scene(
        kind: ProjectileKind,
        resource_manager: &ResourceManager,
        scene: &mut Scene,
        dir: Vector3<f32>,
        position: Vector3<f32>,
        owner: Handle<Node>,
        initial_velocity: Vector3<f32>,
    ) -> Handle<Node> {
        let definition = Self::get_definition(kind);

        let instance_handle = block_on(resource_manager.request_model(definition.model.clone()))
            .unwrap()
            .instantiate(scene);

        let instance_ref = &mut scene.graph[instance_handle];

        instance_ref.local_transform_mut().set_position(position);

        if let Some(projectile) = instance_ref.try_get_script_mut::<Projectile>() {
            projectile.initial_velocity = initial_velocity;
            projectile.dir = dir
                .try_normalize(std::f32::EPSILON)
                .unwrap_or_else(Vector3::y);
            projectile.owner = owner;
        }

        scene.graph.update_hierarchical_data();

        instance_handle
    }
}

impl ScriptTrait for Projectile {
    fn on_init(&mut self, context: &mut ScriptContext) {
        let node = &mut context.scene.graph[context.handle];

        self.last_position = node.global_position();

        if let Some(rigid_body) = node.cast_mut::<RigidBody>() {
            rigid_body.set_lin_vel(self.initial_velocity);
        }
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.definition = Self::get_definition(self.kind);

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

        let position = ctx.scene.graph[ctx.handle].global_position();

        let mut hit = None;

        if self.use_ray_casting {
            hit = Weapon::ray_hit(
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
                .definition
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

            effects::create(
                if hit.hit_actor.is_some() {
                    EffectKind::BloodSpray
                } else {
                    EffectKind::BulletImpact
                },
                &mut ctx.scene.graph,
                ctx.resource_manager,
                hit.position,
                vector_to_quat(hit.normal),
            );

            game.level.as_ref().unwrap().sound_manager.play_sound(
                &mut ctx.scene.graph,
                &self.definition.impact_sound,
                hit.position,
                1.0,
                4.0,
                3.0,
            );

            // Defer destruction.
            ctx.scene.graph[ctx.handle].set_lifetime(Some(0.0));
        }

        // Movement of kinematic projectiles is controlled explicitly.
        if self.definition.is_kinematic {
            let total_velocity = self.dir.scale(self.definition.speed);
            ctx.scene.graph[ctx.handle]
                .local_transform_mut()
                .offset(total_velocity);
        }

        // Reduce initial velocity down to zero over time. This is needed because projectile
        // stabilizes its movement over time.
        self.initial_velocity.follow(&Vector3::default(), 0.15);
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
