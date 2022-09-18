use crate::{
    actor::Actor,
    character::{Character, CharacterCommand},
    control_scheme::ControlButton,
    current_level_mut, current_level_ref,
    door::DoorContainer,
    elevator::{
        call_button::{CallButtonContainer, CallButtonKind},
        ElevatorContainer,
    },
    game_mut, game_ref,
    gui::journal::Journal,
    inventory::Inventory,
    item::ItemKind,
    message::Message,
    player::{
        lower_body::{LowerBodyMachine, LowerBodyMachineInput},
        upper_body::{CombatWeaponKind, UpperBodyMachine, UpperBodyMachineInput},
    },
    weapon::{
        definition::WeaponKind,
        projectile::{ProjectileKind, Shooter},
        try_weapon_ref, weapon_mut, weapon_ref,
    },
    CameraController, Game, Item, MessageSender,
};
use fyrox::{
    animation::{
        machine::{node::blend::IndexedBlendInput, Machine, PoseNode, State},
        Animation,
    },
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        color_gradient::{ColorGradient, ColorGradientBuilder, GradientPoint},
        futures::executor::block_on,
        inspect::prelude::*,
        math::{self, SmoothAngle, Vector3Ext},
        pool::Handle,
        reflect::Reflect,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    event::{DeviceEvent, ElementState, Event, MouseScrollDelta, WindowEvent},
    impl_component_provider,
    material::{shader::SamplerFallback, PropertyValue},
    resource::{model::Model, texture::Texture},
    scene::{
        base::BaseBuilder,
        graph::{map::NodeHandleMap, Graph},
        light::BaseLight,
        node::{Node, TypeUuidProvider},
        sprite::SpriteBuilder,
        Scene,
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
    utils::log::Log,
};
use std::ops::{Deref, DerefMut};

pub mod camera;
mod lower_body;
mod upper_body;

pub struct HitReactionStateDefinition {
    state: Handle<State>,
    hit_reaction_rifle_animation: Handle<Animation>,
    hit_reaction_pistol_animation: Handle<Animation>,
}

pub fn make_hit_reaction_state(
    machine: &mut Machine,
    scene: &mut Scene,
    model: Handle<Node>,
    index: String,
    hit_reaction_rifle_animation_resource: Model,
    hit_reaction_pistol_animation_resource: Model,
) -> HitReactionStateDefinition {
    let hit_reaction_rifle_animation = *hit_reaction_rifle_animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    scene.animations[hit_reaction_rifle_animation]
        .set_speed(1.5)
        .set_enabled(true)
        .set_loop(false);
    let hit_reaction_rifle_animation_node =
        machine.add_node(PoseNode::make_play_animation(hit_reaction_rifle_animation));

    let hit_reaction_pistol_animation = *hit_reaction_pistol_animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    scene.animations[hit_reaction_pistol_animation]
        .set_speed(1.5)
        .set_enabled(false)
        .set_loop(false);
    let hit_reaction_pistol_animation_node =
        machine.add_node(PoseNode::make_play_animation(hit_reaction_pistol_animation));

    let pose_node = PoseNode::make_blend_animations_by_index(
        index,
        vec![
            IndexedBlendInput {
                blend_time: 0.2,
                pose_source: hit_reaction_rifle_animation_node,
            },
            IndexedBlendInput {
                blend_time: 0.2,
                pose_source: hit_reaction_pistol_animation_node,
            },
        ],
    );
    let handle = machine.add_node(pose_node);

    HitReactionStateDefinition {
        state: machine.add_state(State::new("HitReaction", handle)),
        hit_reaction_rifle_animation,
        hit_reaction_pistol_animation,
    }
}

#[derive(Default, Debug)]
pub struct InputController {
    walk_forward: bool,
    walk_backward: bool,
    walk_left: bool,
    walk_right: bool,
    jump: bool,
    yaw: f32,
    pitch: f32,
    aim: bool,
    toss_grenade: bool,
    shoot: bool,
    run: bool,
    action: bool,
    cursor_up: bool,
    cursor_down: bool,
}

impl Deref for Player {
    type Target = Character;

    fn deref(&self) -> &Self::Target {
        &self.character
    }
}

impl DerefMut for Player {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.character
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Visit, Debug)]
pub enum RequiredWeapon {
    None,
    Next,
    Previous,
    Specific(WeaponKind),
}

impl RequiredWeapon {
    fn is_none(self) -> bool {
        matches!(self, RequiredWeapon::None)
    }
}

impl Default for RequiredWeapon {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Clone)]
pub struct PlayerPersistentData {
    pub inventory: Inventory,
    pub health: f32,
    pub current_weapon: u32,
    pub weapons: Vec<WeaponKind>,
}

#[derive(Visit, Reflect, Inspect, Debug)]
pub struct Player {
    character: Character,
    camera_controller: Handle<Node>,
    model_pivot: Handle<Node>,
    #[visit(optional)]
    model_sub_pivot: Handle<Node>,
    model: Handle<Node>,
    model_yaw: SmoothAngle,
    spine_pitch: SmoothAngle,
    spine: Handle<Node>,
    hips: Handle<Node>,
    move_speed: f32,
    weapon_yaw_correction: SmoothAngle,
    weapon_pitch_correction: SmoothAngle,
    weapon_origin: Handle<Node>,
    run_factor: f32,
    target_run_factor: f32,
    in_air_time: f32,
    velocity: Vector3<f32>, // Horizontal velocity, Y is ignored.
    target_velocity: Vector3<f32>,
    weapon_display: Handle<Node>,
    inventory_display: Handle<Node>,
    journal_display: Handle<Node>,
    health_cylinder: Handle<Node>,
    last_health: f32,
    health_color_gradient: ColorGradient,
    v_recoil: SmoothAngle,
    h_recoil: SmoothAngle,
    rig_light: Handle<Node>,

    #[inspect(skip)]
    #[reflect(hidden)]
    item_display: Handle<Node>,

    #[inspect(skip)]
    #[reflect(hidden)]
    lower_body_machine: LowerBodyMachine,

    #[inspect(skip)]
    #[reflect(hidden)]
    upper_body_machine: UpperBodyMachine,

    #[reflect(hidden)]
    #[inspect(skip)]
    weapon_change_direction: RequiredWeapon,

    #[reflect(hidden)]
    #[inspect(skip)]
    pub journal: Journal,

    #[visit(skip)]
    #[reflect(hidden)]
    #[inspect(skip)]
    controller: InputController,
}

impl Default for Player {
    fn default() -> Self {
        Self {
            character: Default::default(),
            rig_light: Default::default(),
            camera_controller: Default::default(),
            inventory_display: Default::default(),
            weapon_origin: Default::default(),
            model: Default::default(),
            controller: Default::default(),
            lower_body_machine: Default::default(),
            health_cylinder: Default::default(),
            upper_body_machine: Default::default(),
            spine: Default::default(),
            hips: Default::default(),
            model_yaw: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 10.0,
            },
            move_speed: 0.65,
            spine_pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 10.0,
            },
            weapon_change_direction: RequiredWeapon::None,
            weapon_yaw_correction: SmoothAngle {
                angle: 0.0,
                target: 30.0f32.to_radians(),
                speed: 10.00, // rad/s
            },
            weapon_pitch_correction: SmoothAngle {
                angle: 0.0,
                target: 10.0f32.to_radians(),
                speed: 10.00, // rad/s
            },
            in_air_time: Default::default(),
            velocity: Default::default(),
            run_factor: Default::default(),
            target_run_factor: Default::default(),
            target_velocity: Default::default(),
            weapon_display: Default::default(),
            last_health: 100.0,
            health_color_gradient: make_color_gradient(),
            item_display: Default::default(),
            v_recoil: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 1.5, // rad/s
            },
            h_recoil: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 1.5, // rad/s
            },
            journal_display: Default::default(),
            journal: Journal::new(),
            model_pivot: Default::default(),
            model_sub_pivot: Default::default(),
        }
    }
}

impl Clone for Player {
    fn clone(&self) -> Self {
        Self {
            character: self.character.clone(),
            camera_controller: self.camera_controller,
            model_pivot: self.model_pivot,
            model_sub_pivot: self.model_sub_pivot,
            model: self.model,
            model_yaw: self.model_yaw.clone(),
            spine_pitch: self.spine_pitch.clone(),
            spine: self.spine,
            hips: self.hips,
            move_speed: self.move_speed,
            weapon_yaw_correction: self.weapon_yaw_correction.clone(),
            weapon_pitch_correction: self.weapon_pitch_correction.clone(),
            weapon_origin: self.weapon_origin,
            run_factor: self.run_factor,
            target_run_factor: self.target_run_factor,
            in_air_time: self.in_air_time,
            velocity: self.velocity,
            target_velocity: self.target_velocity,
            weapon_display: self.weapon_display,
            inventory_display: self.inventory_display,
            journal_display: self.journal_display,
            item_display: self.item_display,
            health_cylinder: self.health_cylinder,
            last_health: self.last_health,
            health_color_gradient: self.health_color_gradient.clone(),
            v_recoil: self.v_recoil.clone(),
            h_recoil: self.h_recoil.clone(),
            rig_light: self.rig_light,
            lower_body_machine: Default::default(),
            upper_body_machine: Default::default(),
            weapon_change_direction: self.weapon_change_direction,
            journal: Default::default(),
            controller: Default::default(),
        }
    }
}

fn make_color_gradient() -> ColorGradient {
    ColorGradientBuilder::new()
        .with_point(GradientPoint::new(0.0, Color::from_rgba(255, 0, 0, 200)))
        .with_point(GradientPoint::new(1.0, Color::from_rgba(0, 255, 0, 200)))
        .build()
}

impl Player {
    pub async fn add_to_scene(
        scene: &mut Scene,
        resource_manager: ResourceManager,
    ) -> Handle<Node> {
        let player = resource_manager
            .request_model("data/models/agent/agent.rgs")
            .await
            .unwrap()
            .instantiate_geometry(scene);

        assert!(scene.graph[player].has_script::<Player>());

        player
    }

    pub fn persistent_data(&self, graph: &Graph) -> PlayerPersistentData {
        PlayerPersistentData {
            inventory: self.inventory.clone(),
            health: self.health,
            current_weapon: self.current_weapon,
            weapons: self
                .weapons
                .iter()
                .map(|w| weapon_ref(*w, graph).kind())
                .collect::<Vec<_>>(),
        }
    }

    pub fn can_be_removed(&self, _scene: &Scene) -> bool {
        self.health <= 0.0
    }

    fn check_items(
        &mut self,
        game: &mut Game,
        scene: &mut Scene,
        resource_manager: &ResourceManager,
    ) {
        let sender = &game.message_sender;
        let items = &game.level.as_ref().unwrap().items;
        for &item_handle in items.iter() {
            if let Some(item_node) = scene.graph.try_get(item_handle) {
                let item = item_node.try_get_script::<Item>().unwrap();
                let self_position = scene.graph[self.body].global_position();
                let item_position = item_node.global_position();

                let distance = (item_position - self_position).norm();
                if distance < 0.75 {
                    game.item_display.sync_to_model(
                        resource_manager.clone(),
                        item.get_kind(),
                        item.stack_size,
                    );

                    if self.controller.action {
                        self.push_command(CharacterCommand::PickupItem(item_handle));
                        sender.send(Message::SyncInventory);

                        self.controller.action = false;
                    }

                    let display = &mut scene.graph[self.item_display];
                    display
                        .local_transform_mut()
                        .set_position(item_position + Vector3::new(0.0, 0.2, 0.0));
                    display.set_visibility(true);

                    break;
                }
            }
        }
    }

    fn check_doors(
        &mut self,
        self_handle: Handle<Actor>,
        scene: &Scene,
        door_container: &DoorContainer,
        sender: &MessageSender,
    ) {
        if self.controller.action {
            door_container.check_actor(
                self.position(&scene.graph),
                self_handle,
                &scene.graph,
                sender,
            );
        }
    }

    fn check_elevators(
        &self,
        scene: &Scene,
        elevator_container: &ElevatorContainer,
        call_button_container: &CallButtonContainer,
        sender: &MessageSender,
    ) {
        let graph = &scene.graph;
        let self_position = graph[self.body].global_position();

        for (handle, elevator) in elevator_container.pair_iter() {
            // Handle floors.
            let elevator_position = graph[elevator.node].global_position();
            if (elevator_position - self_position).norm() < 0.75 && self.controller.action {
                let last_index = elevator.points.len().saturating_sub(1) as u32;
                if elevator.current_floor == last_index {
                    sender.send(Message::CallElevator {
                        elevator: handle,
                        floor: 0,
                    });
                } else if elevator.current_floor == 0 {
                    sender.send(Message::CallElevator {
                        elevator: handle,
                        floor: last_index,
                    });
                }
            }

            // Handle call buttons
            for &call_button_handle in elevator.call_buttons.iter() {
                let call_button = &call_button_container[call_button_handle];

                let button_position = graph[call_button.node].global_position();

                let distance = (button_position - self_position).norm();
                if distance < 0.75 {
                    if let CallButtonKind::FloorSelector = call_button.kind {
                        let new_floor = if self.controller.cursor_down {
                            Some(call_button.floor.saturating_sub(1))
                        } else if self.controller.cursor_up {
                            Some(
                                call_button
                                    .floor
                                    .saturating_add(1)
                                    .min((elevator.points.len() as u32).saturating_sub(1)),
                            )
                        } else {
                            None
                        };

                        if let Some(new_floor) = new_floor {
                            sender.send(Message::SetCallButtonFloor {
                                call_button: call_button_handle,
                                floor: new_floor,
                            });
                        }
                    }

                    if self.controller.action {
                        sender.send(Message::CallElevator {
                            elevator: handle,
                            floor: call_button.floor,
                        });
                    }
                }
            }
        }
    }

    fn handle_jump_signal(&self, scene: &mut Scene, dt: f32) -> Option<f32> {
        let mut new_y_vel = None;
        while let Some(event) = scene
            .animations
            .get_mut(self.lower_body_machine.jump_animation)
            .pop_event()
        {
            if event.signal_id == LowerBodyMachine::JUMP_SIGNAL
                && (self.lower_body_machine.machine.active_transition()
                    == self.lower_body_machine.idle_to_jump
                    || self.lower_body_machine.machine.active_transition()
                        == self.lower_body_machine.walk_to_jump
                    || self.lower_body_machine.machine.active_state()
                        == self.lower_body_machine.jump_state)
            {
                new_y_vel = Some(3.0 * dt);
            }
        }
        new_y_vel
    }

    fn handle_weapon_grab_signal(&mut self, scene: &mut Scene) {
        while let Some(event) = scene
            .animations
            .get_mut(self.upper_body_machine.grab_animation)
            .pop_event()
        {
            if event.signal_id == UpperBodyMachine::GRAB_WEAPON_SIGNAL {
                match self.weapon_change_direction {
                    RequiredWeapon::None => (),
                    RequiredWeapon::Next => self.next_weapon(&mut scene.graph),
                    RequiredWeapon::Previous => self.prev_weapon(&mut scene.graph),
                    RequiredWeapon::Specific(kind) => {
                        self.push_command(CharacterCommand::SelectWeapon(kind));
                    }
                }

                self.weapon_change_direction = RequiredWeapon::None;
            }
        }
    }

    fn handle_put_back_weapon_end_signal(&self, scene: &mut Scene) {
        while let Some(event) = scene
            .animations
            .get_mut(self.upper_body_machine.put_back_animation)
            .pop_event()
        {
            if event.signal_id == UpperBodyMachine::PUT_BACK_WEAPON_END_SIGNAL {
                scene
                    .animations
                    .get_mut(self.upper_body_machine.grab_animation)
                    .set_enabled(true);
            }
        }
    }

    fn handle_toss_grenade_signal(
        &mut self,
        self_handle: Handle<Actor>,
        scene: &mut Scene,
        sender: &MessageSender,
    ) {
        while let Some(event) = scene
            .animations
            .get_mut(self.upper_body_machine.toss_grenade_animation)
            .pop_event()
        {
            if event.signal_id == UpperBodyMachine::TOSS_GRENADE_SIGNAL {
                let position = scene.graph[self.weapon_pivot].global_position();

                let direction = scene
                    .graph
                    .try_get(self.camera_controller)
                    .and_then(|c| c.try_get_script::<CameraController>())
                    .map(|c| scene.graph[c.camera()].look_vector())
                    .unwrap_or_default();

                if self.inventory.try_extract_exact_items(ItemKind::Grenade, 1) == 1 {
                    sender.send(Message::CreateProjectile {
                        kind: ProjectileKind::Grenade,
                        position,
                        direction,
                        initial_velocity: direction.scale(15.0),
                        shooter: Shooter::Actor(self_handle),
                    });
                }
            }
        }
    }

    fn update_velocity(&mut self, scene: &Scene, can_move: bool, dt: f32) {
        // We're using model pivot's angles for movement instead of rigid body, because
        // camera controller is attached to the body and we'd rotate rigid body, the
        // camera would rotate too, but we don't want this.
        let model_pivot = &scene.graph[self.model_pivot];

        let look_vector = model_pivot
            .look_vector()
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        let side_vector = model_pivot
            .side_vector()
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_else(Vector3::x);

        self.target_velocity = Vector3::default();

        if self.controller.walk_right {
            self.target_velocity -= side_vector;
        }
        if self.controller.walk_left {
            self.target_velocity += side_vector;
        }
        if self.controller.walk_forward {
            self.target_velocity += look_vector;
        }
        if self.controller.walk_backward {
            self.target_velocity -= look_vector;
        }

        let speed = if can_move {
            math::lerpf(self.move_speed, self.move_speed * 4.0, self.run_factor) * dt
        } else {
            0.0
        };

        self.target_velocity = self
            .target_velocity
            .try_normalize(f32::EPSILON)
            .map(|v| v.scale(speed))
            .unwrap_or_default();

        self.velocity.follow(&self.target_velocity, 0.15);
    }

    fn current_weapon_kind(&self, graph: &Graph) -> CombatWeaponKind {
        if self.current_weapon().is_some() {
            match weapon_ref(self.current_weapon(), graph).kind() {
                WeaponKind::M4
                | WeaponKind::Ak47
                | WeaponKind::PlasmaRifle
                | WeaponKind::RailGun => CombatWeaponKind::Rifle,
                WeaponKind::Glock => CombatWeaponKind::Pistol,
            }
        } else {
            CombatWeaponKind::Rifle
        }
    }

    fn should_be_stunned(&self) -> bool {
        self.last_health - self.health >= 15.0
    }

    fn stun(&mut self, scene: &mut Scene) {
        for &animation in self
            .lower_body_machine
            .hit_reaction_animations()
            .iter()
            .chain(self.upper_body_machine.hit_reaction_animations().iter())
        {
            scene.animations[animation].set_enabled(true).rewind();
        }

        self.last_health = self.health;
    }

    fn is_walking(&self) -> bool {
        self.controller.walk_backward
            || self.controller.walk_forward
            || self.controller.walk_right
            || self.controller.walk_left
    }

    fn update_health_cylinder(&self, scene: &mut Scene) {
        let mesh = scene.graph[self.health_cylinder].as_mesh_mut();
        let color = self.health_color_gradient.get_color(self.health / 100.0);
        let surface = mesh.surfaces_mut().first_mut().unwrap();
        let mut material = surface.material().lock();
        Log::verify(material.set_property(
            &ImmutableString::new("diffuseColor"),
            PropertyValue::Color(color),
        ));
        Log::verify(material.set_property(
            &ImmutableString::new("emissionStrength"),
            PropertyValue::Vector3(color.as_frgb().scale(10.0)),
        ));
        drop(material);
        scene.graph[self.rig_light]
            .query_component_mut::<BaseLight>()
            .unwrap()
            .set_color(color);
    }

    fn update_animation_machines(
        &mut self,
        dt: f32,
        scene: &mut Scene,
        is_walking: bool,
        is_jumping: bool,
        has_ground_contact: bool,
        sender: &MessageSender,
    ) {
        let weapon_kind = self.current_weapon_kind(&scene.graph);

        let should_be_stunned = self.should_be_stunned();
        if should_be_stunned {
            self.stun(scene);
        }

        self.lower_body_machine.apply(
            scene,
            dt,
            LowerBodyMachineInput {
                is_walking,
                is_jumping,
                has_ground_contact: self.in_air_time <= 0.3,
                run_factor: self.run_factor,
                is_dead: self.is_dead(),
                should_be_stunned: false,
                weapon_kind,
            },
            sender,
            has_ground_contact,
            self.capsule_collider,
        );

        self.upper_body_machine.apply(
            scene,
            dt,
            self.hips,
            UpperBodyMachineInput {
                is_walking,
                is_jumping,
                has_ground_contact: self.in_air_time <= 0.3,
                is_aiming: self.controller.aim,
                toss_grenade: self.controller.toss_grenade,
                weapon_kind,
                change_weapon: self.weapon_change_direction != RequiredWeapon::None,
                run_factor: self.run_factor,
                is_dead: self.is_dead(),
                should_be_stunned,
            },
        );
    }

    fn calculate_model_angle(&self) -> f32 {
        if self.controller.aim {
            if self.controller.walk_left {
                if self.controller.walk_backward {
                    -45.0
                } else {
                    45.0
                }
            } else if self.controller.walk_right {
                if self.controller.walk_backward {
                    45.0
                } else {
                    -45.0
                }
            } else {
                0.0
            }
        } else if self.controller.walk_left {
            if self.controller.walk_forward {
                45.0
            } else if self.controller.walk_backward {
                135.0
            } else {
                90.0
            }
        } else if self.controller.walk_right {
            if self.controller.walk_forward {
                -45.0
            } else if self.controller.walk_backward {
                -135.0
            } else {
                -90.0
            }
        } else if self.controller.walk_backward {
            180.0
        } else {
            0.0
        }
    }

    fn update_shooting(&mut self, scene: &mut Scene, dt: f32, elapsed_time: f32) {
        self.v_recoil.update(dt);
        self.h_recoil.update(dt);

        if let Some(&current_weapon_handle) = self
            .character
            .weapons
            .get(self.character.current_weapon as usize)
        {
            if self.upper_body_machine.machine.active_state() == self.upper_body_machine.aim_state {
                weapon_mut(current_weapon_handle, &mut scene.graph)
                    .laser_sight_mut()
                    .enabled = true;

                let ammo_indicator_offset = weapon_ref(current_weapon_handle, &scene.graph)
                    .definition
                    .ammo_indicator_offset();
                let weapon_display = &mut scene.graph[self.weapon_display];
                weapon_display.set_visibility(true);
                weapon_display
                    .local_transform_mut()
                    .set_position(ammo_indicator_offset);

                if self.controller.shoot
                    && weapon_ref(current_weapon_handle, &scene.graph).can_shoot(elapsed_time)
                {
                    let ammo_per_shot = weapon_ref(current_weapon_handle, &scene.graph)
                        .definition
                        .ammo_consumption_per_shot;

                    if self
                        .inventory
                        .try_extract_exact_items(ItemKind::Ammo, ammo_per_shot)
                        == ammo_per_shot
                    {
                        weapon_mut(current_weapon_handle, &mut scene.graph).request_shot(None);

                        if let Some(camera_controller) = scene
                            .graph
                            .try_get_mut(self.camera_controller)
                            .and_then(|c| c.try_get_script_mut::<CameraController>())
                        {
                            camera_controller.request_shake_camera();
                        }
                        self.v_recoil.set_target(
                            weapon_ref(current_weapon_handle, &scene.graph)
                                .definition
                                .gen_v_recoil_angle(),
                        );
                        self.h_recoil.set_target(
                            weapon_ref(current_weapon_handle, &scene.graph)
                                .definition
                                .gen_h_recoil_angle(),
                        );
                    }
                }
            } else {
                weapon_mut(current_weapon_handle, &mut scene.graph)
                    .laser_sight_mut()
                    .enabled = false;
                scene.graph[self.weapon_display].set_visibility(false);
            }
        }
    }

    fn can_move(&self) -> bool {
        self.lower_body_machine.machine.active_state() != self.lower_body_machine.fall_state
            && self.lower_body_machine.machine.active_state() != self.lower_body_machine.land_state
    }

    fn apply_weapon_angular_correction(&mut self, scene: &mut Scene, can_move: bool, dt: f32) {
        if self.controller.aim {
            let (pitch_correction, yaw_correction) =
                if let Some(weapon) = try_weapon_ref(self.current_weapon(), &scene.graph) {
                    (
                        weapon.definition.pitch_correction,
                        weapon.definition.yaw_correction,
                    )
                } else {
                    (-12.0f32, -4.0f32)
                };

            self.weapon_yaw_correction
                .set_target(yaw_correction.to_radians());
            self.weapon_pitch_correction
                .set_target(pitch_correction.to_radians());
        } else {
            self.weapon_yaw_correction.set_target(30.0f32.to_radians());
            self.weapon_pitch_correction.set_target(8.0f32.to_radians());
        }

        if can_move {
            let yaw_correction_angle = self.weapon_yaw_correction.update(dt).angle();
            let pitch_correction_angle = self.weapon_pitch_correction.update(dt).angle();
            scene.graph[self.weapon_pivot]
                .local_transform_mut()
                .set_rotation(
                    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), yaw_correction_angle)
                        * UnitQuaternion::from_axis_angle(
                            &Vector3::x_axis(),
                            pitch_correction_angle,
                        ),
                );
        }
    }

    fn is_running(&self, scene: &Scene) -> bool {
        !self.is_dead()
            && self.controller.run
            && !self.controller.aim
            && !self.lower_body_machine.is_stunned(scene)
    }

    pub fn is_aiming(&self) -> bool {
        self.controller.aim
    }

    pub fn is_completely_dead(&self, scene: &Scene) -> bool {
        self.is_dead()
            && (scene.animations[self.upper_body_machine.dying_animation].has_ended()
                || scene.animations[self.lower_body_machine.dying_animation].has_ended())
    }

    pub fn resolve(
        &mut self,
        scene: &mut Scene,
        display_texture: Texture,
        inventory_texture: Texture,
        item_texture: Texture,
        journal_texture: Texture,
    ) {
        Log::verify(
            scene.graph[self.weapon_display]
                .as_mesh_mut()
                .surfaces_mut()
                .first_mut()
                .unwrap()
                .material()
                .lock()
                .set_property(
                    &ImmutableString::new("diffuseTexture"),
                    PropertyValue::Sampler {
                        value: Some(display_texture),
                        fallback: SamplerFallback::White,
                    },
                ),
        );

        Log::verify(
            scene.graph[self.inventory_display]
                .as_mesh_mut()
                .surfaces_mut()
                .first_mut()
                .unwrap()
                .material()
                .lock()
                .set_property(
                    &ImmutableString::new("diffuseTexture"),
                    PropertyValue::Sampler {
                        value: Some(inventory_texture),
                        fallback: SamplerFallback::White,
                    },
                ),
        );

        Log::verify(
            scene.graph[self.journal_display]
                .as_mesh_mut()
                .surfaces_mut()
                .first_mut()
                .unwrap()
                .material()
                .lock()
                .set_property(
                    &ImmutableString::new("diffuseTexture"),
                    PropertyValue::Sampler {
                        value: Some(journal_texture),
                        fallback: SamplerFallback::White,
                    },
                ),
        );

        scene.graph[self.item_display]
            .as_sprite_mut()
            .set_texture(Some(item_texture));

        self.health_color_gradient = make_color_gradient();
    }
}

impl_component_provider!(Player, character: Character);

impl TypeUuidProvider for Player {
    fn type_uuid() -> Uuid {
        uuid!("50a07510-893d-476f-aad2-fcfb0845807f")
    }
}

impl ScriptTrait for Player {
    fn on_init(&mut self, context: &mut ScriptContext) {
        self.lower_body_machine = block_on(LowerBodyMachine::new(
            context.scene,
            self.model,
            context.resource_manager.clone(),
        ));

        self.upper_body_machine = block_on(UpperBodyMachine::new(
            context.scene,
            self.model,
            context.resource_manager.clone(),
        ));

        self.item_display = SpriteBuilder::new(BaseBuilder::new().with_depth_offset(0.05))
            .with_size(0.1)
            .build(&mut context.scene.graph);

        let game = game_ref(context.plugins);
        self.resolve(
            context.scene,
            game.weapon_display.render_target.clone(),
            game.inventory_interface.render_target.clone(),
            game.item_display.render_target.clone(),
            game.journal_display.render_target.clone(),
        );

        // Add default weapon.
        self.push_command(CharacterCommand::AddWeapon(WeaponKind::Glock));

        current_level_mut(context.plugins).unwrap().player = context.handle;
    }

    fn on_deinit(&mut self, context: &mut ScriptDeinitContext) {
        if let Some(level) = current_level_mut(context.plugins) {
            level.player = Handle::NONE;
        }
    }

    fn on_os_event(&mut self, event: &Event<()>, context: &mut ScriptContext) {
        let game = game_ref(context.plugins);
        let control_scheme = &game.control_scheme;
        let sender = &game.message_sender;

        let button_state = match event {
            Event::WindowEvent { event, .. } => {
                if let WindowEvent::KeyboardInput { input, .. } = event {
                    input
                        .virtual_keycode
                        .map(|vk| (ControlButton::Key(vk), input.state))
                } else {
                    None
                }
            }
            Event::DeviceEvent { event, .. } => match event {
                &DeviceEvent::MouseWheel { delta } => match delta {
                    MouseScrollDelta::LineDelta(_, y) => {
                        if y < 0.0 {
                            Some((ControlButton::WheelDown, ElementState::Pressed))
                        } else {
                            Some((ControlButton::WheelUp, ElementState::Pressed))
                        }
                    }
                    MouseScrollDelta::PixelDelta(delta) => {
                        if delta.y < 0.0 {
                            Some((ControlButton::WheelDown, ElementState::Pressed))
                        } else {
                            Some((ControlButton::WheelUp, ElementState::Pressed))
                        }
                    }
                },
                &DeviceEvent::Button { button, state } => {
                    Some((ControlButton::Mouse(button as u16), state))
                }
                DeviceEvent::MouseMotion { delta } => {
                    let mouse_sens = control_scheme.mouse_sens * context.dt;
                    self.controller.yaw -= (delta.0 as f32) * mouse_sens;
                    let pitch_direction = if control_scheme.mouse_y_inverse {
                        -1.0
                    } else {
                        1.0
                    };
                    self.controller.pitch = (self.controller.pitch
                        + pitch_direction * (delta.1 as f32) * mouse_sens)
                        .max(-90.0f32.to_radians())
                        .min(90.0f32.to_radians());
                    None
                }
                _ => None,
            },
            _ => None,
        };

        let can_change_weapon = self.weapon_change_direction.is_none()
            && context.scene.animations[self.upper_body_machine.grab_animation].has_ended()
            && self.weapons.len() > 1;

        let current_weapon_kind = if self.current_weapon().is_some() {
            Some(weapon_ref(self.current_weapon(), &context.scene.graph).kind())
        } else {
            None
        };

        let mut weapon_change_direction = None;

        if let Some((button, state)) = button_state {
            if button == control_scheme.aim.button {
                self.controller.aim = state == ElementState::Pressed;
                if state == ElementState::Pressed {
                    context.scene.graph[self.inventory_display].set_visibility(false);
                    context.scene.graph[self.journal_display].set_visibility(false);
                }
            } else if button == control_scheme.move_forward.button {
                self.controller.walk_forward = state == ElementState::Pressed;
            } else if button == control_scheme.move_backward.button {
                self.controller.walk_backward = state == ElementState::Pressed;
            } else if button == control_scheme.move_left.button {
                self.controller.walk_left = state == ElementState::Pressed;
            } else if button == control_scheme.move_right.button {
                self.controller.walk_right = state == ElementState::Pressed;
            } else if button == control_scheme.jump.button {
                let jump_anim = context
                    .scene
                    .animations
                    .get(self.lower_body_machine.jump_animation);
                let can_jump = !jump_anim.is_enabled() || jump_anim.has_ended();

                if state == ElementState::Pressed && can_jump {
                    // Rewind jump animation to beginning before jump.
                    context
                        .scene
                        .animations
                        .get_mut(self.lower_body_machine.jump_animation)
                        .set_enabled(true)
                        .rewind();
                    context
                        .scene
                        .animations
                        .get_mut(self.upper_body_machine.jump_animation)
                        .set_enabled(true)
                        .rewind();
                }

                self.controller.jump = state == ElementState::Pressed && can_jump;
            } else if button == control_scheme.run.button {
                self.controller.run = state == ElementState::Pressed;
            } else if button == control_scheme.flash_light.button {
                if state == ElementState::Pressed {
                    let current_weapon = self.current_weapon();
                    if current_weapon.is_some() {
                        weapon_mut(current_weapon, &mut context.scene.graph).switch_flash_light();
                    }
                }
            } else if button == control_scheme.grab_ak47.button && can_change_weapon {
                if current_weapon_kind.map_or(false, |k| k != WeaponKind::Ak47) {
                    weapon_change_direction = Some(RequiredWeapon::Specific(WeaponKind::Ak47));
                }
            } else if button == control_scheme.grab_m4.button && can_change_weapon {
                if current_weapon_kind.map_or(false, |k| k != WeaponKind::M4) {
                    weapon_change_direction = Some(RequiredWeapon::Specific(WeaponKind::M4));
                }
            } else if button == control_scheme.grab_plasma_gun.button && can_change_weapon {
                if current_weapon_kind.map_or(false, |k| k != WeaponKind::PlasmaRifle) {
                    weapon_change_direction =
                        Some(RequiredWeapon::Specific(WeaponKind::PlasmaRifle));
                }
            } else if button == control_scheme.grab_pistol.button && can_change_weapon {
                if current_weapon_kind.map_or(false, |k| k != WeaponKind::Glock) {
                    weapon_change_direction = Some(RequiredWeapon::Specific(WeaponKind::Glock));
                }
            } else if button == control_scheme.next_weapon.button {
                if state == ElementState::Pressed
                    && self.current_weapon < self.weapons.len().saturating_sub(1) as u32
                    && can_change_weapon
                {
                    weapon_change_direction = Some(RequiredWeapon::Next);
                }
            } else if button == control_scheme.prev_weapon.button {
                if state == ElementState::Pressed && self.current_weapon > 0 && can_change_weapon {
                    weapon_change_direction = Some(RequiredWeapon::Previous);
                }
            } else if button == control_scheme.toss_grenade.button {
                if self.inventory.item_count(ItemKind::Grenade) > 0 {
                    self.controller.toss_grenade = state == ElementState::Pressed;
                    if state == ElementState::Pressed {
                        context
                            .scene
                            .animations
                            .get_mut(self.upper_body_machine.toss_grenade_animation)
                            .set_enabled(true)
                            .rewind();
                    }
                }
            } else if button == control_scheme.shoot.button {
                self.controller.shoot = state == ElementState::Pressed;
            } else if button == control_scheme.cursor_up.button {
                self.controller.cursor_up = state == ElementState::Pressed;
            } else if button == control_scheme.cursor_down.button {
                self.controller.cursor_down = state == ElementState::Pressed;
            } else if button == control_scheme.action.button {
                self.controller.action = state == ElementState::Pressed;
            } else if button == control_scheme.inventory.button
                && state == ElementState::Pressed
                && !self.controller.aim
            {
                context.scene.graph[self.journal_display].set_visibility(false);

                let inventory = &mut context.scene.graph[self.inventory_display];
                let new_visibility = !inventory.visibility();
                inventory.set_visibility(new_visibility);
                if new_visibility {
                    sender.send(Message::SyncInventory);
                }
            } else if button == control_scheme.journal.button
                && state == ElementState::Pressed
                && !self.controller.aim
            {
                context.scene.graph[self.inventory_display].set_visibility(false);

                let journal = &mut context.scene.graph[self.journal_display];
                let new_visibility = !journal.visibility();
                journal.set_visibility(new_visibility);
                if new_visibility {
                    sender.send(Message::SyncJournal);
                }
            }
        }

        if let Some(weapon_change_direction) = weapon_change_direction {
            self.weapon_change_direction = weapon_change_direction;

            context
                .scene
                .animations
                .get_mut(self.upper_body_machine.put_back_animation)
                .rewind();

            context
                .scene
                .animations
                .get_mut(self.upper_body_machine.grab_animation)
                .set_enabled(false)
                .rewind();
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = game_ref(ctx.plugins);
        let level = current_level_ref(ctx.plugins).unwrap();
        let sender = &game.message_sender;

        self.process_commands(
            ctx.scene,
            ctx.handle,
            ctx.resource_manager,
            &game.message_sender,
        );

        self.update_health_cylinder(ctx.scene);

        let has_ground_contact = self.has_ground_contact(&ctx.scene.graph);
        let is_walking = self.is_walking();
        let is_jumping = has_ground_contact && self.controller.jump;

        self.update_animation_machines(
            ctx.dt,
            ctx.scene,
            is_walking,
            is_jumping,
            has_ground_contact,
            sender,
        );

        let is_running = self.is_running(ctx.scene);

        if !self.is_dead() {
            if is_running {
                self.target_run_factor = 1.0;
            } else {
                self.target_run_factor = 0.0;
            }
            self.run_factor += (self.target_run_factor - self.run_factor) * 0.1;

            let can_move = self.can_move();
            self.update_velocity(ctx.scene, can_move, ctx.dt);
            let new_y_vel = self.handle_jump_signal(ctx.scene, ctx.dt);
            self.handle_weapon_grab_signal(ctx.scene);
            self.handle_put_back_weapon_end_signal(ctx.scene);
            self.handle_toss_grenade_signal(Default::default(), ctx.scene, sender);

            let body = ctx.scene.graph[self.body].as_rigid_body_mut();
            body.set_ang_vel(Default::default());
            if let Some(new_y_vel) = new_y_vel {
                body.set_lin_vel(Vector3::new(
                    self.velocity.x / ctx.dt,
                    new_y_vel / ctx.dt,
                    self.velocity.z / ctx.dt,
                ));
            } else {
                body.set_lin_vel(Vector3::new(
                    self.velocity.x / ctx.dt,
                    body.lin_vel().y,
                    self.velocity.z / ctx.dt,
                ));
            }

            if self.controller.aim {
                self.spine_pitch.set_target(self.controller.pitch);
            } else {
                self.spine_pitch.set_target(0.0);
            }

            self.spine_pitch.update(ctx.dt);

            if can_move && (is_walking || self.controller.aim) {
                // Since we have free camera while not moving, we have to sync rotation of pivot
                // with rotation of camera so character will start moving in look direction.
                ctx.scene.graph[self.model_pivot]
                    .local_transform_mut()
                    .set_rotation(UnitQuaternion::from_axis_angle(
                        &Vector3::y_axis(),
                        self.controller.yaw,
                    ));

                // Apply additional rotation to model - it will turn in front of walking direction.
                let angle = self.calculate_model_angle();

                self.model_yaw.set_target(angle.to_radians()).update(ctx.dt);

                let mut additional_hips_rotation = Default::default();
                if self.controller.aim {
                    ctx.scene.graph[self.model_sub_pivot]
                        .local_transform_mut()
                        .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0));

                    let spine_transform = ctx.scene.graph[self.spine].local_transform_mut();
                    let spine_rotation = **spine_transform.rotation();
                    spine_transform.set_rotation(
                        spine_rotation
                            * UnitQuaternion::from_axis_angle(
                                &Vector3::x_axis(),
                                self.spine_pitch.angle,
                            )
                            * UnitQuaternion::from_axis_angle(
                                &Vector3::y_axis(),
                                -(self.model_yaw.angle + 37.5f32.to_radians()),
                            ),
                    );
                    additional_hips_rotation =
                        UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.model_yaw.angle);
                } else {
                    ctx.scene.graph[self.model_sub_pivot]
                        .local_transform_mut()
                        .set_rotation(UnitQuaternion::from_axis_angle(
                            &Vector3::y_axis(),
                            self.model_yaw.angle,
                        ));

                    ctx.scene.graph[self.spine]
                        .local_transform_mut()
                        .set_rotation(
                            UnitQuaternion::from_axis_angle(
                                &Vector3::x_axis(),
                                self.spine_pitch.angle,
                            ) * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0),
                        );
                }

                ctx.scene.graph[self.hips]
                    .local_transform_mut()
                    .set_rotation(
                        additional_hips_rotation
                            * UnitQuaternion::from_axis_angle(
                                &Vector3::x_axis(),
                                math::lerpf(
                                    5.0f32.to_radians(),
                                    17.0f32.to_radians(),
                                    self.run_factor,
                                ),
                            ),
                    );

                let walk_dir = if self.controller.aim && self.controller.walk_backward {
                    -1.0
                } else {
                    1.0
                };

                for &animation in &[
                    self.lower_body_machine.walk_animation,
                    self.upper_body_machine.walk_animation,
                    self.lower_body_machine.run_animation,
                    self.upper_body_machine.run_animation,
                ] {
                    ctx.scene.animations.get_mut(animation).set_speed(walk_dir);
                }
            }

            self.apply_weapon_angular_correction(ctx.scene, can_move, ctx.dt);

            if has_ground_contact {
                self.in_air_time = 0.0;
            } else {
                self.in_air_time += ctx.dt;
            }

            if !has_ground_contact {
                for &land_animation in &[
                    self.lower_body_machine.land_animation,
                    self.upper_body_machine.land_animation,
                ] {
                    ctx.scene.animations.get_mut(land_animation).rewind();
                }
            }

            ctx.scene.graph[self.item_display].set_visibility(false);

            self.check_doors(
                Default::default(),
                ctx.scene,
                &level.doors_container,
                sender,
            );
            self.check_elevators(ctx.scene, &level.elevators, &level.call_buttons, sender);
            self.update_shooting(ctx.scene, ctx.dt, ctx.elapsed_time);
            self.check_items(game_mut(ctx.plugins), ctx.scene, ctx.resource_manager);

            let spine_transform = ctx.scene.graph[self.spine].local_transform_mut();
            let rotation = **spine_transform.rotation();
            spine_transform.set_rotation(
                rotation
                    * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.v_recoil.angle())
                    * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.h_recoil.angle()),
            );
        } else {
            for &dying_animation in &[
                self.lower_body_machine.dying_animation,
                self.upper_body_machine.dying_animation,
            ] {
                ctx.scene
                    .animations
                    .get_mut(dying_animation)
                    .set_enabled(true);
            }

            // Lock player on the place he died.
            let body = ctx.scene.graph[self.body].as_rigid_body_mut();
            body.set_ang_vel(Default::default());
            body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));
        }
    }

    fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        self.character.remap_handles(old_new_mapping);
        old_new_mapping
            .map(&mut self.model)
            .map(&mut self.spine)
            .map(&mut self.hips)
            .map(&mut self.weapon_origin)
            .map(&mut self.weapon_display)
            .map(&mut self.inventory_display)
            .map(&mut self.journal_display)
            .map(&mut self.item_display)
            .map(&mut self.health_cylinder)
            .map(&mut self.rig_light)
            .map(&mut self.model_pivot)
            .map(&mut self.model_sub_pivot);
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
