use crate::{
    bot::{
        behavior::{BehaviorContext, BotBehavior},
        state_machine::{StateMachine, StateMachineInput},
    },
    character::{Character, CharacterMessage, CharacterMessageData},
    door::{door_mut, door_ref, DoorContainer},
    utils::{self, is_probability_event_occurred, BodyImpactHandler, ResourceProxy},
    weapon::WeaponMessage,
    Game, Level,
};
use fyrox::{
    core::{
        algebra::{Point3, UnitQuaternion, Vector3},
        arrayvec::ArrayVec,
        color::Color,
        math::SmoothAngle,
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::{Visit, VisitResult, Visitor},
        TypeUuidProvider,
    },
    impl_component_provider, rand,
    rand::prelude::SliceRandom,
    scene::{
        self,
        debug::SceneDrawingContext,
        graph::{
            physics::{Intersection, RayCastOptions},
            Graph,
        },
        node::Node,
        rigidbody::RigidBody,
        sound::SoundBufferResource,
        Scene,
    },
    script::{
        ScriptContext, ScriptDeinitContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait,
    },
    utils::navmesh::{NavmeshAgent, NavmeshAgentBuilder},
};
use serde::Deserialize;
use std::ops::{Deref, DerefMut};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

mod behavior;
mod state_machine;

#[derive(
    Deserialize,
    Copy,
    Clone,
    PartialOrd,
    PartialEq,
    Ord,
    Eq,
    Hash,
    Debug,
    Visit,
    Reflect,
    AsRefStr,
    EnumString,
    EnumVariantNames,
)]
#[repr(u32)]
pub enum BotHostility {
    Everyone = 0,
    OtherSpecies = 1,
    Player = 2,
}

#[derive(Debug, Visit, Default, Clone)]
pub struct Target {
    position: Vector3<f32>,
    handle: Handle<Node>,
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Bot {
    #[reflect(hidden)]
    target: Option<Target>,
    model: Handle<Node>,
    character: Character,
    #[reflect(hidden)]
    #[visit(skip)]
    state_machine: StateMachine,
    pub restoration_time: f32,
    hips: Handle<Node>,
    #[reflect(hidden)]
    agent: NavmeshAgent,
    head_exploded: bool,
    #[visit(skip)]
    #[reflect(hidden)]
    pub impact_handler: BodyImpactHandler,
    #[reflect(hidden)]
    behavior: BotBehavior,
    v_recoil: SmoothAngle,
    h_recoil: SmoothAngle,
    spine: Handle<Node>,
    threaten_timeout: f32,
    #[visit(optional)]
    head: Handle<Node>,
    #[visit(optional)]
    animation_player: Handle<Node>,
    #[visit(optional)]
    absm: Handle<Node>,
    #[visit(optional)]
    yaw: SmoothAngle,
    #[visit(optional)]
    pitch: SmoothAngle,
    #[visit(optional)]
    pub walk_speed: f32,
    #[visit(optional)]
    pub v_aim_angle_hack: f32,
    #[visit(optional)]
    pub h_aim_angle_hack: f32,
    #[visit(optional)]
    pub close_combat_distance: f32,
    #[visit(optional)]
    pub pain_sounds: Vec<ResourceProxy<SoundBufferResource>>,
    #[visit(optional)]
    pub scream_sounds: Vec<ResourceProxy<SoundBufferResource>>,
    #[visit(optional)]
    pub idle_sounds: Vec<ResourceProxy<SoundBufferResource>>,
    #[visit(optional)]
    pub attack_sounds: Vec<ResourceProxy<SoundBufferResource>>,
    #[visit(optional)]
    pub hostility: BotHostility,
}

impl_component_provider!(Bot, character: Character);

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
            model: Default::default(),
            target: Default::default(),
            state_machine: Default::default(),
            restoration_time: 0.0,
            hips: Default::default(),
            agent: Default::default(),
            head_exploded: false,
            impact_handler: Default::default(),
            behavior: Default::default(),
            v_recoil: Default::default(),
            h_recoil: Default::default(),
            spine: Default::default(),
            threaten_timeout: 0.0,
            head: Default::default(),
            animation_player: Default::default(),
            absm: Default::default(),
            walk_speed: 1.2,
            v_aim_angle_hack: 0.0,
            h_aim_angle_hack: 0.0,
            close_combat_distance: 1.0,
            pain_sounds: Default::default(),
            scream_sounds: Default::default(),
            idle_sounds: Default::default(),
            attack_sounds: Default::default(),
            hostility: BotHostility::Player,
            yaw: SmoothAngle {
                angle: f32::NAN, // Nan means undefined.
                target: 0.0,
                speed: 270.0f32.to_radians(),
            },
            pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 270.0f32.to_radians(),
            },
        }
    }
}

impl Bot {
    #[allow(clippy::unnecessary_to_owned)] // false positive
    fn check_doors(&mut self, scene: &mut Scene, door_container: &DoorContainer) {
        if let Some(target) = self.target.as_ref() {
            let mut query_storage = ArrayVec::<Intersection, 64>::new();

            let position = self.position(&scene.graph);
            let ray_direction = target.position - position;

            scene.graph.physics.cast_ray(
                RayCastOptions {
                    ray_origin: Point3::from(position),
                    ray_direction,
                    max_len: ray_direction.norm(),
                    groups: Default::default(),
                    sort_results: true,
                },
                &mut query_storage,
            );

            for intersection in query_storage {
                for &door_handle in &door_container.doors {
                    let door = door_ref(door_handle, &scene.graph);

                    let close_enough = position.metric_distance(&door.initial_position()) < 1.25;
                    if !close_enough {
                        continue;
                    }

                    for child in scene.graph[door_handle].children().to_vec() {
                        if let Some(rigid_body) = scene.graph[child].cast::<RigidBody>() {
                            for collider in rigid_body.children().to_vec() {
                                if collider == intersection.collider {
                                    door_mut(door_handle, &mut scene.graph)
                                        .try_open(Some(&self.inventory));
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn can_be_removed(&self, scene: &Scene) -> bool {
        if let Some(upper_body_layer) = self.state_machine.upper_body_layer(&scene.graph) {
            let animations =
                utils::fetch_animation_container_ref(&scene.graph, self.animation_player);
            upper_body_layer
                .is_all_animations_of_state_ended(self.state_machine.dead_state, animations)
        } else {
            false
        }
    }

    pub fn debug_draw(&self, context: &mut SceneDrawingContext) {
        for pts in self.agent.path().windows(2) {
            let a = pts[0];
            let b = pts[1];
            context.add_line(scene::debug::Line {
                begin: a,
                end: b,
                color: Color::from_rgba(255, 0, 0, 255),
            });
        }

        // context.draw_frustum(&self.frustum, Color::from_rgba(0, 200, 0, 255)); TODO
    }

    pub fn set_target(&mut self, handle: Handle<Node>, position: Vector3<f32>) {
        self.target = Some(Target { position, handle });
    }

    pub fn blow_up_head(&mut self, _graph: &mut Graph) {
        self.head_exploded = true;

        // TODO: Add effect.
    }

    pub fn on_actor_removed(&mut self, handle: Handle<Node>) {
        if let Some(target) = self.target.as_ref() {
            if target.handle == handle {
                self.target = None;
            }
        }
    }
}

impl TypeUuidProvider for Bot {
    fn type_uuid() -> Uuid {
        uuid!("15a8ecd6-a09f-4c5d-b9f9-b7f0e8a44ac9")
    }
}

impl ScriptTrait for Bot {
    fn on_init(&mut self, context: &mut ScriptContext) {
        self.agent = NavmeshAgentBuilder::new()
            .with_position(context.scene.graph[context.handle].global_position())
            .with_speed(self.walk_speed)
            .build();
        self.behavior = BotBehavior::new(self.spine, self.close_combat_distance);

        Level::try_get_mut(context.plugins)
            .unwrap()
            .actors
            .push(context.handle);
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.state_machine = StateMachine::new(self.absm, &ctx.scene.graph).unwrap();
        ctx.message_dispatcher
            .subscribe_to::<CharacterMessage>(ctx.handle);
        ctx.message_dispatcher
            .subscribe_to::<WeaponMessage>(ctx.handle);
    }

    fn on_deinit(&mut self, context: &mut ScriptDeinitContext) {
        if let Some(level) = Level::try_get_mut(context.plugins) {
            if let Some(position) = level.actors.iter().position(|a| *a == context.node_handle) {
                level.actors.remove(position);
            }
        }
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        if let Some(char_message) = message.downcast_ref::<CharacterMessage>() {
            if char_message.character != ctx.handle {
                return;
            }

            let level = Level::try_get(ctx.plugins).unwrap();

            self.character.on_character_message(
                &char_message.data,
                ctx.scene,
                ctx.handle,
                ctx.message_sender,
                &level.sound_manager,
            );

            if let CharacterMessageData::Damage {
                dealer,
                amount,
                hitbox,
                critical_hit_probability: critical_shot_probability,
                position,
            } = char_message.data
            {
                if let Some((character_handle, character)) = dealer.as_character(&ctx.scene.graph) {
                    self.set_target(character_handle, character.position(&ctx.scene.graph));
                }

                if let Some(hitbox) = hitbox {
                    // Handle critical head shots.
                    let critical_head_shot_probability = critical_shot_probability.clamp(0.0, 1.0); // * 100.0%
                    if hitbox.is_head
                        && is_probability_event_occurred(critical_head_shot_probability)
                    {
                        self.damage(amount * 1000.0);

                        self.blow_up_head(&mut ctx.scene.graph);
                    }

                    if let Some(position) = position {
                        self.impact_handler.handle_impact(
                            ctx.scene,
                            hitbox.bone,
                            position.point,
                            position.direction,
                        );
                    }
                }

                // Prevent spamming with grunt sounds.
                if self.last_health - self.health > 20.0 && !self.is_dead() {
                    self.last_health = self.health;
                    self.restoration_time = 0.8;

                    if let Some(grunt_sound) = self.pain_sounds.choose(&mut rand::thread_rng()) {
                        let position = self.position(&ctx.scene.graph);

                        level.sound_manager.try_play_sound_buffer(
                            &mut ctx.scene.graph,
                            grunt_sound.0.as_ref(),
                            position,
                            0.8,
                            1.0,
                            0.6,
                        );
                    }
                }
            }
        } else if let Some(weapon_message) = message.downcast_ref() {
            self.character
                .on_weapon_message(weapon_message, &mut ctx.scene.graph);
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = Game::game_ref(ctx.plugins);
        let level = Level::try_get(ctx.plugins).unwrap();

        let movement_speed_factor;
        let is_attacking;
        let is_moving;
        let is_aiming;
        let attack_animation_index;
        let is_screaming;
        {
            let mut behavior_ctx = BehaviorContext {
                scene: ctx.scene,
                actors: &level.actors,
                bot_handle: ctx.handle,
                sender: &game.message_sender,
                dt: ctx.dt,
                elapsed_time: ctx.elapsed_time,
                state_machine: &self.state_machine,
                target: &mut self.target,
                character: &mut self.character,
                agent: &mut self.agent,
                impact_handler: &self.impact_handler,
                model: self.model,
                restoration_time: self.restoration_time,
                v_recoil: &mut self.v_recoil,
                h_recoil: &mut self.h_recoil,
                move_speed: self.walk_speed,
                threaten_timeout: &mut self.threaten_timeout,
                sound_manager: &level.sound_manager,
                script_message_sender: ctx.message_sender,
                navmesh: level.navmesh,
                yaw: &mut self.yaw,
                pitch: &mut self.pitch,

                // Output
                hostility: self.hostility,
                v_aim_angle_hack: self.v_aim_angle_hack,
                h_aim_angle_hack: self.h_aim_angle_hack,
                animation_player: self.animation_player,
                attack_animation_index: 0,
                movement_speed_factor: 1.0,
                is_moving: false,
                is_attacking: false,
                is_aiming_weapon: false,
                is_screaming: false,
                attack_sounds: &self.attack_sounds,
            };

            self.behavior.tree.tick(&mut behavior_ctx);

            movement_speed_factor = behavior_ctx.movement_speed_factor;
            is_attacking = behavior_ctx.is_attacking;
            is_moving = behavior_ctx.is_moving;
            is_aiming = behavior_ctx.is_aiming_weapon;
            attack_animation_index = behavior_ctx.attack_animation_index;
            is_screaming = behavior_ctx.is_screaming;
        }

        self.restoration_time -= ctx.dt;
        self.threaten_timeout -= ctx.dt;

        self.check_doors(ctx.scene, &level.doors_container);

        self.state_machine.apply(
            ctx.scene,
            StateMachineInput {
                walk: is_moving,
                scream: is_screaming,
                dead: self.is_dead(),
                movement_speed_factor,
                attack: is_attacking,
                attack_animation_index: attack_animation_index as u32,
                aim: is_aiming,
            },
        );
        self.impact_handler.update_and_apply(ctx.dt, ctx.scene);

        self.v_recoil.update(ctx.dt);
        self.h_recoil.update(ctx.dt);

        let spine_transform = ctx.scene.graph[self.spine].local_transform_mut();
        let rotation = **spine_transform.rotation();
        spine_transform.set_rotation(
            rotation
                * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.v_recoil.angle())
                * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.h_recoil.angle()),
        );

        if self.head_exploded {
            if let Some(head) = ctx.scene.graph.try_get_mut(self.head) {
                head.local_transform_mut()
                    .set_scale(Vector3::new(0.0, 0.0, 0.0));
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

pub fn try_get_bot_mut(handle: Handle<Node>, graph: &mut Graph) -> Option<&mut Bot> {
    graph
        .try_get_mut(handle)
        .and_then(|b| b.try_get_script_mut::<Bot>())
}
