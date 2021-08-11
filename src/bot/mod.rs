use crate::bot::behavior::{BehaviorContext, BotBehavior};
use crate::{
    actor::{Actor, TargetDescriptor},
    bot::{
        lower_body::{LowerBodyMachine, LowerBodyMachineInput},
        upper_body::{UpperBodyMachine, UpperBodyMachineInput},
    },
    character::{find_hit_boxes, Character},
    inventory::{Inventory, ItemEntry},
    item::ItemKind,
    level::UpdateContext,
    utils::BodyImpactHandler,
    weapon::projectile::Damage,
    CollisionGroups,
};
use rg3d::core::math::SmoothAngle;
use rg3d::core::rand::Rng;
use rg3d::{
    animation::machine::{Machine, PoseNode},
    core::{
        algebra::{Isometry3, Translation3, UnitQuaternion, Vector3},
        color::Color,
        pool::Handle,
        rand::seq::IteratorRandom,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::{MaterialSearchOptions, ResourceManager},
    lazy_static::lazy_static,
    physics::{
        dynamics::{CoefficientCombineRule, RigidBodyBuilder, RigidBodyType},
        geometry::{ColliderBuilder, InteractionGroups},
    },
    rand,
    scene::{
        self, base::BaseBuilder, graph::Graph, node::Node, transform::TransformBuilder, Scene,
        SceneDrawingContext,
    },
    utils::{
        log::{Log, MessageKind},
        navmesh::{NavmeshAgent, NavmeshAgentBuilder},
    },
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    fs::File,
    ops::{Deref, DerefMut},
};

mod behavior;
mod lower_body;
mod upper_body;

#[derive(Deserialize, Copy, Clone, PartialEq, Eq, Hash, Debug, Visit)]
#[repr(i32)]
pub enum BotKind {
    Mutant = 0,
    Parasite = 1,
    Zombie = 2,
}

impl BotKind {
    pub fn description(self) -> &'static str {
        match self {
            BotKind::Mutant => "Mutant",
            BotKind::Parasite => "Parasite",
            BotKind::Zombie => "Zombie",
        }
    }
}

#[derive(Deserialize, Copy, Clone, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[repr(u32)]
pub enum BotHostility {
    Everyone = 0,
    OtherSpecies = 1,
    Player = 2,
}

#[derive(Debug, Visit, Default, Clone)]
pub struct Target {
    position: Vector3<f32>,
    handle: Handle<Actor>,
}

pub struct Bot {
    target: Option<Target>,
    pub kind: BotKind,
    model: Handle<Node>,
    character: Character,
    pub definition: &'static BotDefinition,
    lower_body_machine: LowerBodyMachine,
    upper_body_machine: UpperBodyMachine,
    pub restoration_time: f32,
    hips: Handle<Node>,
    agent: NavmeshAgent,
    head_exploded: bool,
    pub impact_handler: BodyImpactHandler,
    behavior: BotBehavior,
    v_recoil: SmoothAngle,
    h_recoil: SmoothAngle,
    spine: Handle<Node>,
    move_speed: f32,
    target_move_speed: f32,
    threaten_timeout: f32,
}

impl Deref for Bot {
    type Target = Character;

    fn deref(&self) -> &Self::Target {
        &self.character
    }
}

impl DerefMut for Bot {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.character
    }
}

impl Default for Bot {
    fn default() -> Self {
        Self {
            character: Default::default(),
            kind: BotKind::Mutant,
            model: Default::default(),
            target: Default::default(),
            definition: Self::get_definition(BotKind::Mutant),
            lower_body_machine: Default::default(),
            upper_body_machine: Default::default(),
            restoration_time: 0.0,
            hips: Default::default(),
            agent: Default::default(),
            head_exploded: false,
            impact_handler: Default::default(),
            behavior: Default::default(),
            v_recoil: Default::default(),
            h_recoil: Default::default(),
            spine: Default::default(),
            move_speed: 0.0,
            target_move_speed: 0.0,
            threaten_timeout: 0.0,
        }
    }
}

#[derive(Deserialize)]
pub struct AttackAnimationDefinition {
    path: String,
    stick_timestamp: f32,
    timestamp: f32,
    damage: Damage,
    speed: f32,
}

#[derive(Deserialize)]
pub struct BotDefinition {
    pub scale: f32,
    pub health: f32,
    pub walk_speed: f32,
    pub weapon_scale: f32,
    pub model: String,
    pub weapon_hand_name: String,
    pub left_leg_name: String,
    pub right_leg_name: String,
    pub spine: String,
    pub head_name: String,
    pub hips: String,
    pub v_aim_angle_hack: f32,
    pub can_use_weapons: bool,
    pub close_combat_distance: f32,
    pub pain_sounds: Vec<String>,
    pub scream_sounds: Vec<String>,
    pub idle_sounds: Vec<String>,
    pub attack_sounds: Vec<String>,
    pub hostility: BotHostility,

    // Animations.
    pub idle_animation: String,
    pub scream_animation: String,
    pub attack_animations: Vec<AttackAnimationDefinition>,
    pub walk_animation: String,
    pub aim_animation: String,
    pub dying_animation: String,
}

#[derive(Deserialize, Default)]
pub struct BotDefinitionsContainer {
    map: HashMap<BotKind, BotDefinition>,
}

impl BotDefinitionsContainer {
    pub fn new() -> Self {
        let file = File::open("data/configs/bots.ron").unwrap();
        ron::de::from_reader(file).unwrap()
    }
}

lazy_static! {
    static ref DEFINITIONS: BotDefinitionsContainer = BotDefinitionsContainer::new();
}

impl Bot {
    pub fn get_definition(kind: BotKind) -> &'static BotDefinition {
        DEFINITIONS.map.get(&kind).unwrap()
    }

    pub async fn new(
        kind: BotKind,
        resource_manager: ResourceManager,
        scene: &mut Scene,
        position: Vector3<f32>,
        rotation: UnitQuaternion<f32>,
    ) -> Self {
        let definition = Self::get_definition(kind);

        let body_height = 0.55;
        let body_radius = 0.16;

        let model = resource_manager
            .request_model(&definition.model, MaterialSearchOptions::UsePathDirectly)
            .await
            .unwrap()
            .instantiate_geometry(scene);

        scene.graph[model]
            .local_transform_mut()
            .set_position(Vector3::new(0.0, -body_height * 0.5 - body_radius, 0.0))
            .set_scale(Vector3::new(
                definition.scale,
                definition.scale,
                definition.scale,
            ));

        let spine = scene.graph.find_by_name(model, &definition.spine);
        if spine.is_none() {
            Log::writeln(
                MessageKind::Warning,
                "Spine bone not found, bot won't aim vertically!".to_owned(),
            );
        }

        let pivot = BaseBuilder::new()
            .with_children(&[model])
            .build(&mut scene.graph);

        let body = scene.physics.add_body(
            RigidBodyBuilder::new(RigidBodyType::Dynamic)
                .lock_rotations()
                .position(Isometry3 {
                    translation: Translation3 { vector: position },
                    rotation,
                })
                .build(),
        );
        scene.physics.add_collider(
            ColliderBuilder::capsule_y(body_height * 0.5, body_radius)
                .friction(0.0)
                .friction_combine_rule(CoefficientCombineRule::Min)
                .collision_groups(InteractionGroups::new(
                    CollisionGroups::ActorCapsule as u32,
                    0xFFFF,
                ))
                .build(),
            &body,
        );

        scene.physics_binder.bind(pivot, body);

        let hand = scene
            .graph
            .find_by_name(model, &definition.weapon_hand_name);
        let wpn_scale = definition.weapon_scale * (1.0 / definition.scale);
        let weapon_pivot = BaseBuilder::new()
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_scale(Vector3::new(wpn_scale, wpn_scale, wpn_scale))
                    .with_local_rotation(
                        UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -90.0f32.to_radians())
                            * UnitQuaternion::from_axis_angle(
                                &Vector3::z_axis(),
                                -90.0f32.to_radians(),
                            ),
                    )
                    .build(),
            )
            .build(&mut scene.graph);

        scene.graph.link_nodes(weapon_pivot, hand);

        let hips = scene.graph.find_by_name(model, &definition.hips);

        let lower_body_machine =
            LowerBodyMachine::new(resource_manager.clone(), &definition, model, scene).await;
        let upper_body_machine =
            UpperBodyMachine::new(resource_manager.clone(), definition, model, scene, hips).await;

        let possible_item = [
            (ItemKind::Ammo, 10),
            (ItemKind::Medkit, 1),
            (ItemKind::Medpack, 1),
        ];
        let mut items =
            if let Some((item, count)) = possible_item.iter().choose(&mut rand::thread_rng()) {
                vec![ItemEntry {
                    kind: *item,
                    amount: *count,
                }]
            } else {
                Default::default()
            };

        if definition.can_use_weapons {
            items.push(ItemEntry {
                kind: ItemKind::Ammo,
                amount: rand::thread_rng().gen_range(32..96),
            });
        }

        Self {
            character: Character {
                pivot,
                body: Some(body),
                weapon_pivot,
                health: definition.health,
                hit_boxes: find_hit_boxes(pivot, scene),
                inventory: Inventory::from_inner(items),
                ..Default::default()
            },
            hips,
            definition,
            model,
            kind,
            lower_body_machine,
            upper_body_machine,
            spine,
            agent: NavmeshAgentBuilder::new()
                .with_position(position)
                .with_speed(definition.walk_speed)
                .build(),
            behavior: BotBehavior::new(spine, definition),
            ..Default::default()
        }
    }

    pub fn can_be_removed(&self, scene: &Scene) -> bool {
        scene
            .animations
            .get(self.upper_body_machine.dying_animation)
            .has_ended()
    }

    pub fn debug_draw(&self, context: &mut SceneDrawingContext) {
        for pts in self.agent.path().windows(2) {
            let a = pts[0];
            let b = pts[1];
            context.add_line(scene::Line {
                begin: a,
                end: b,
                color: Color::from_rgba(255, 0, 0, 255),
            });
        }

        // context.draw_frustum(&self.frustum, Color::from_rgba(0, 200, 0, 255)); TODO
    }

    pub fn set_target(&mut self, handle: Handle<Actor>, position: Vector3<f32>) {
        self.target = Some(Target { position, handle });
    }

    pub fn update(
        &mut self,
        self_handle: Handle<Actor>,
        context: &mut UpdateContext,
        targets: &[TargetDescriptor],
    ) {
        let mut behavior_context = BehaviorContext {
            scene: context.scene,
            bot_handle: self_handle,
            targets,
            weapons: context.weapons,
            sender: context.sender,
            time: context.time,
            navmesh: context.navmesh,
            upper_body_machine: &self.upper_body_machine,
            lower_body_machine: &self.lower_body_machine,
            target: &mut self.target,
            definition: self.definition,
            character: &mut self.character,
            kind: self.kind,
            agent: &mut self.agent,
            impact_handler: &self.impact_handler,
            model: self.model,
            restoration_time: self.restoration_time,
            v_recoil: &mut self.v_recoil,
            h_recoil: &mut self.h_recoil,
            target_move_speed: &mut self.target_move_speed,
            move_speed: self.move_speed,
            threaten_timeout: &mut self.threaten_timeout,

            // Output
            attack_animation_index: 0,
            movement_speed_factor: 1.0,
            is_moving: false,
            is_attacking: false,
            is_aiming_weapon: false,
            is_screaming: false,
        };

        self.behavior.tree.tick(&mut behavior_context);

        let time = behavior_context.time;
        let movement_speed_factor = behavior_context.movement_speed_factor;
        let is_attacking = behavior_context.is_attacking;
        let is_moving = behavior_context.is_moving;
        let is_aiming = behavior_context.is_aiming_weapon;
        let attack_animation_index = behavior_context.attack_animation_index;
        let is_screaming = behavior_context.is_screaming;

        drop(behavior_context);

        self.restoration_time -= time.delta;
        self.move_speed += (self.target_move_speed - self.move_speed) * 0.1;
        self.threaten_timeout -= time.delta;

        self.lower_body_machine.apply(
            context.scene,
            time.delta,
            LowerBodyMachineInput {
                walk: is_moving,
                scream: is_screaming,
                dead: self.is_dead(),
                movement_speed_factor,
            },
        );

        self.upper_body_machine.apply(
            context.scene,
            time,
            UpperBodyMachineInput {
                attack: is_attacking,
                walk: is_moving,
                scream: is_screaming,
                dead: self.is_dead(),
                aim: is_aiming,
                attack_animation_index: attack_animation_index as u32,
            },
        );
        self.impact_handler
            .update_and_apply(time.delta, context.scene);

        self.v_recoil.update(time.delta);
        self.h_recoil.update(time.delta);

        let spine_transform = context.scene.graph[self.spine].local_transform_mut();
        let rotation = **spine_transform.rotation();
        spine_transform.set_rotation(
            rotation
                * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.v_recoil.angle())
                * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.h_recoil.angle()),
        );

        if self.head_exploded {
            let head = context
                .scene
                .graph
                .find_by_name(self.model, &self.definition.head_name);
            if head.is_some() {
                context.scene.graph[head]
                    .local_transform_mut()
                    .set_scale(Vector3::new(0.0, 0.0, 0.0));
            }
        }
    }

    pub fn blow_up_head(&mut self, _graph: &mut Graph) {
        self.head_exploded = true;

        // TODO: Add effect.
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        self.upper_body_machine.clean_up(scene);
        self.lower_body_machine.clean_up(scene);
        self.character.clean_up(scene);
    }

    pub fn on_actor_removed(&mut self, handle: Handle<Actor>) {
        if let Some(target) = self.target.as_ref() {
            if target.handle == handle {
                self.target = None;
            }
        }
    }
}

fn clean_machine(machine: &Machine, scene: &mut Scene) {
    for node in machine.nodes() {
        if let PoseNode::PlayAnimation(node) = node {
            scene.animations.remove(node.animation);
        }
    }
}

impl Visit for Bot {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.kind.visit("Kind", visitor)?;
        self.definition = Self::get_definition(self.kind);
        self.character.visit("Character", visitor)?;
        self.model.visit("Model", visitor)?;
        self.target.visit("Target", visitor)?;
        self.lower_body_machine
            .visit("LocomotionMachine", visitor)?;
        self.upper_body_machine.visit("AimMachine", visitor)?;
        self.restoration_time.visit("RestorationTime", visitor)?;
        self.hips.visit("Hips", visitor)?;
        self.agent.visit("Agent", visitor)?;
        self.head_exploded.visit("HeadExploded", visitor)?;
        self.behavior.visit("Behavior", visitor)?;
        self.v_recoil.visit("VRecoil", visitor)?;
        self.h_recoil.visit("HRecoil", visitor)?;
        self.spine.visit("Spine", visitor)?;

        visitor.leave_region()
    }
}
