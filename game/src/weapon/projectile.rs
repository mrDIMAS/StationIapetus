use crate::{
    character::{try_get_character_mut, CharacterCommand},
    current_level_ref, effects,
    effects::EffectKind,
    game_ref,
    message::Message,
    weapon::{sight::SightReaction, Hit},
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
        node::{Node, TypeUuidProvider},
        rigidbody::RigidBody,
        sprite::Sprite,
        Scene,
    },
    script::{ScriptContext, ScriptTrait},
};
use serde::Deserialize;
use std::{
    collections::{HashMap, HashSet},
    fs::File,
};
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
    dir: Vector3<f32>,
    lifetime: f32,
    rotation_angle: f32,
    pub owner: Handle<Node>,
    initial_velocity: Vector3<f32>,
    /// Position of projectile on the previous frame, it is used to simulate
    /// continuous intersection detection from fast moving projectiles.
    last_position: Vector3<f32>,

    #[visit(skip)]
    #[reflect(hidden)]
    definition: &'static ProjectileDefinition,

    #[visit(skip)]
    #[reflect(hidden)]
    hits: HashSet<Hit>,
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
            lifetime: 0.0,
            rotation_angle: 0.0,
            owner: Default::default(),
            initial_velocity: Default::default(),
            last_position: Default::default(),
            definition: Self::get_definition(ProjectileKind::Plasma),
            hits: Default::default(),
        }
    }
}

#[derive(Deserialize, Debug)]
pub struct ProjectileDefinition {
    damage: Damage,
    speed: f32,
    lifetime: f32,
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

        instance_handle
    }

    pub fn is_dead(&self) -> bool {
        self.lifetime <= 0.0
    }

    pub fn kill(&mut self) {
        self.lifetime = 0.0;
    }
}

impl ScriptTrait for Projectile {
    fn on_init(&mut self, context: &mut ScriptContext) {
        let definition = Self::get_definition(self.kind);

        self.lifetime = definition.lifetime;

        let node = &mut context.scene.graph[context.handle];

        self.last_position = node.global_position();

        if let Some(rigid_body) = node.cast_mut::<RigidBody>() {
            rigid_body.set_lin_vel(self.initial_velocity);
        }
    }

    fn on_start(&mut self, _context: &mut ScriptContext) {
        self.definition = Self::get_definition(self.kind);
    }

    fn on_update(&mut self, context: &mut ScriptContext) {
        let game = game_ref(context.plugins);

        // Fetch current position of projectile.
        let (position, collider) =
            if let Some(body) = context.scene.graph[context.handle].cast::<RigidBody>() {
                let position = body.global_position();
                let collider = body
                    .children()
                    .iter()
                    .cloned()
                    .find(|c| context.scene.graph[*c].is_collider())
                    .unwrap_or_default();
                (position, collider)
            } else {
                (
                    context.scene.graph[context.handle].global_position(),
                    Handle::NONE,
                )
            };

        let ray_hit = Weapon::ray_hit(
            self.last_position,
            position,
            self.owner,
            &current_level_ref(context.plugins).unwrap().actors,
            &mut context.scene.graph,
            collider,
        );

        let (effect_position, effect_normal, effect_kind) = if let Some(hit) = ray_hit {
            let position = hit.position;
            let normal = hit.normal;
            let blood_effect = hit.actor.is_some();

            self.hits.insert(hit);
            self.kill();

            (
                position,
                normal,
                if blood_effect {
                    EffectKind::BloodSpray
                } else {
                    EffectKind::BulletImpact
                },
            )
        } else {
            (
                context.scene.graph[context.handle].global_position(),
                Vector3::y(),
                EffectKind::BulletImpact,
            )
        };

        // Movement of kinematic projectiles are controlled explicitly.
        if self.definition.is_kinematic {
            let total_velocity = self.dir.scale(self.definition.speed);
            context.scene.graph[context.handle]
                .local_transform_mut()
                .offset(total_velocity);
        }

        // TODO: Replace with animation.
        if let Some(sprite) = context.scene.graph[context.handle].cast_mut::<Sprite>() {
            sprite.set_rotation(self.rotation_angle);
            self.rotation_angle += 1.5;
        }

        // Reduce initial velocity down to zero over time. This is needed because projectile
        // stabilizes its movement over time.
        self.initial_velocity.follow(&Vector3::default(), 0.15);

        self.lifetime -= context.dt;

        if self.lifetime <= 0.0 {
            effects::create(
                effect_kind,
                &mut context.scene.graph,
                context.resource_manager,
                effect_position,
                vector_to_quat(effect_normal),
            );

            game.level.as_ref().unwrap().sound_manager.play_sound(
                &mut context.scene.graph,
                &self.definition.impact_sound,
                effect_position,
                1.0,
                4.0,
                3.0,
            );
        }

        for hit in self.hits.drain() {
            let damage = self
                .definition
                .damage
                .scale(hit.hit_box.map_or(1.0, |h| h.damage_factor));

            let critical_shot_probability =
                context
                    .scene
                    .graph
                    .try_get_mut(self.owner)
                    .map_or(0.0, |owner_node| {
                        if let Some(weapon) = owner_node.try_get_script_mut::<Weapon>() {
                            if hit.actor.is_some() {
                                weapon.set_sight_reaction(SightReaction::HitDetected);
                            }
                            weapon.definition.base_critical_shot_probability
                        } else if owner_node.has_script::<Turret>() {
                            0.01
                        } else {
                            0.0
                        }
                    });

            match damage {
                Damage::Splash { radius, amount } => {
                    game.message_sender.send(Message::ApplySplashDamage {
                        amount,
                        radius,
                        center: position,
                        who: hit.who,
                        critical_shot_probability,
                    })
                }
                Damage::Point(amount) => {
                    if let Some(character) =
                        try_get_character_mut(hit.actor, &mut context.scene.graph)
                    {
                        character.push_command(CharacterCommand::Damage {
                            who: hit.who,
                            hitbox: hit.hit_box,
                            amount,
                            critical_shot_probability,
                        });
                    }
                }
            }
        }

        self.last_position = position;

        if self.is_dead() {
            context.scene.graph.remove_node(context.handle);
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
