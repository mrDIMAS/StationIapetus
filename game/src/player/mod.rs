use crate::{
    character::{Character, CharacterCommand},
    control_scheme::ControlButton,
    current_level_mut, current_level_ref,
    door::{door_mut, DoorContainer},
    elevator::call_button::{CallButton, CallButtonKind},
    game_mut, game_ref,
    gui::journal::Journal,
    inventory::Inventory,
    level::item::ItemKind,
    message::Message,
    player::state_machine::{CombatWeaponKind, StateMachine, StateMachineInput},
    sound::SoundManager,
    utils,
    weapon::{
        definition::WeaponKind,
        projectile::{Projectile, ProjectileKind},
        try_weapon_ref, weapon_mut, weapon_ref,
    },
    CameraController, Elevator, Game, Item,
};
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        color::Color,
        color_gradient::{ColorGradient, ColorGradientBuilder, GradientPoint},
        math::{self, SmoothAngle, Vector3Ext},
        pool::Handle,
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    engine::resource_manager::ResourceManager,
    event::{DeviceEvent, ElementState, Event, MouseScrollDelta, WindowEvent},
    impl_component_provider,
    material::{shader::SamplerFallback, PropertyValue},
    resource::texture::Texture,
    scene::{
        animation::absm::AnimationBlendingStateMachine,
        base::BaseBuilder,
        graph::Graph,
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
mod state_machine;

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

#[derive(Visit, Reflect, Debug)]
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
    machine: Handle<Node>,

    #[visit(optional)]
    animation_player: Handle<Node>,

    #[reflect(hidden)]
    item_display: Handle<Node>,

    #[reflect(hidden)]
    #[visit(skip)]
    state_machine: StateMachine,

    #[reflect(hidden)]
    weapon_change_direction: RequiredWeapon,

    #[reflect(hidden)]
    pub journal: Journal,

    #[visit(skip)]
    #[reflect(hidden)]
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
            health_cylinder: Default::default(),
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
            animation_player: Default::default(),
            machine: Default::default(),
            state_machine: Default::default(),
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
            weapon_change_direction: self.weapon_change_direction,
            journal: Default::default(),
            controller: Default::default(),
            animation_player: self.animation_player,
            machine: self.machine,
            state_machine: self.state_machine.clone(),
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
            .instantiate(scene);

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

    fn check_doors(&mut self, scene: &mut Scene, door_container: &DoorContainer) {
        let self_position = self.position(&scene.graph);

        if self.controller.action {
            for &door_handle in &door_container.doors {
                let door = door_mut(door_handle, &mut scene.graph);
                let close_enough = self_position.metric_distance(&door.initial_position()) < 1.25;
                if close_enough {
                    let has_key = self.inventory.has_key();
                    door.try_open(has_key);
                }
            }
        }
    }

    fn check_elevators(&self, scene: &mut Scene, elevators: &[Handle<Node>]) {
        let graph = &mut scene.graph;
        let self_position = graph[self.body].global_position();

        for &elevator_handle in elevators.iter() {
            let mut graph_multiborrow = graph.begin_multi_borrow::<32>();

            let elevator_node = graph_multiborrow.try_get(elevator_handle).unwrap();
            let elevator_position = elevator_node.global_position();

            let elevator_script = elevator_node.try_get_script_mut::<Elevator>().unwrap();

            // Handle floors.
            let mut requested_floor = None;
            if (elevator_position - self_position).norm() < 0.75 && self.controller.action {
                let last_index = elevator_script.point_handles.len().saturating_sub(1) as u32;
                if elevator_script.current_floor == last_index {
                    requested_floor = Some(0);
                } else if elevator_script.current_floor == 0 {
                    requested_floor = Some(last_index);
                }
            }

            // Handle call buttons
            for &call_button_handle in elevator_script.call_buttons.iter() {
                if let Some(call_button_node) = graph_multiborrow.try_get(*call_button_handle) {
                    let button_position = call_button_node.global_position();

                    let call_button_script =
                        call_button_node.try_get_script_mut::<CallButton>().unwrap();

                    let distance = (button_position - self_position).norm();
                    if distance < 0.75 {
                        if let CallButtonKind::FloorSelector = call_button_script.kind {
                            let new_floor = if self.controller.cursor_down {
                                Some(call_button_script.floor.saturating_sub(1))
                            } else if self.controller.cursor_up {
                                Some(call_button_script.floor.saturating_add(1).min(
                                    (elevator_script.point_handles.len() as u32).saturating_sub(1),
                                ))
                            } else {
                                None
                            };

                            if let Some(new_floor) = new_floor {
                                call_button_script.floor = new_floor;
                            }
                        }

                        if self.controller.action {
                            requested_floor = Some(call_button_script.floor);
                        }
                    }
                } else {
                    Log::warn(format!(
                        "Unable to get call button {:?}!",
                        call_button_handle
                    ));
                }
            }

            if let Some(requested_floor) = requested_floor {
                elevator_script.call_to(requested_floor);
            }
        }
    }

    fn handle_jump_signal(&self, scene: &mut Scene, dt: f32) -> Option<f32> {
        let mut new_y_vel = None;
        let animations_container =
            utils::fetch_animation_container_mut(&mut scene.graph, self.animation_player);
        let mut events = animations_container
            .get_mut(self.state_machine.jump_animation)
            .take_events();
        while let Some(event) = events.pop_front() {
            if let Some(layer) = self.state_machine.lower_body_layer(&scene.graph) {
                let active_transition = layer.active_transition();
                if event.name == StateMachine::JUMP_SIGNAL
                    && (active_transition == self.state_machine.idle_to_jump
                        || active_transition == self.state_machine.walk_to_jump
                        || layer.active_state() == self.state_machine.jump_state)
                {
                    new_y_vel = Some(3.0 * dt);
                }
            }
        }
        new_y_vel
    }

    fn handle_weapon_grab_signal(&mut self, scene: &mut Scene) {
        let animations_container =
            utils::fetch_animation_container_mut(&mut scene.graph, self.animation_player);
        let mut events = animations_container
            .get_mut(self.state_machine.grab_animation)
            .take_events();
        while let Some(event) = events.pop_front() {
            if event.name == StateMachine::GRAB_WEAPON_SIGNAL {
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
        let animations_container =
            utils::fetch_animation_container_mut(&mut scene.graph, self.animation_player);
        while let Some(event) = animations_container
            .get_mut(self.state_machine.put_back_animation)
            .pop_event()
        {
            if event.name == StateMachine::PUT_BACK_WEAPON_END_SIGNAL {
                animations_container
                    .get_mut(self.state_machine.grab_animation)
                    .set_enabled(true);
            }
        }
    }

    fn handle_toss_grenade_signal(
        &mut self,
        self_handle: Handle<Node>,
        scene: &mut Scene,
        resource_manager: &ResourceManager,
    ) {
        let animations_container =
            utils::fetch_animation_container_mut(&mut scene.graph, self.animation_player);
        let mut events = animations_container
            .get_mut(self.state_machine.toss_grenade_animation)
            .take_events();
        while let Some(event) = events.pop_front() {
            if event.name == StateMachine::TOSS_GRENADE_SIGNAL {
                let position = scene.graph[self.weapon_pivot].global_position();

                let direction = scene
                    .graph
                    .try_get(self.camera_controller)
                    .and_then(|c| c.try_get_script::<CameraController>())
                    .map(|c| scene.graph[c.camera()].look_vector())
                    .unwrap_or_default();

                if self.inventory.try_extract_exact_items(ItemKind::Grenade, 1) == 1 {
                    Projectile::add_to_scene(
                        ProjectileKind::Grenade,
                        resource_manager,
                        scene,
                        direction,
                        position,
                        self_handle,
                        direction.scale(15.0),
                    );
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
        let animations_container =
            utils::fetch_animation_container_mut(&mut scene.graph, self.animation_player);
        for &animation in self
            .state_machine
            .hit_reaction_animations()
            .iter()
            .chain(self.state_machine.hit_reaction_animations().iter())
        {
            animations_container[animation].set_enabled(true).rewind();
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
        scene: &mut Scene,
        is_walking: bool,
        is_jumping: bool,
        has_ground_contact: bool,
        sound_manager: &SoundManager,
    ) {
        let weapon_kind = self.current_weapon_kind(&scene.graph);

        let should_be_stunned = self.should_be_stunned();
        if should_be_stunned {
            self.stun(scene);
        }

        self.state_machine.apply(StateMachineInput {
            is_walking,
            is_jumping,
            has_ground_contact: self.in_air_time <= 0.3,
            is_aiming: self.controller.aim,
            run_factor: self.run_factor,
            is_dead: self.is_dead(),
            should_be_stunned,
            machine: self.machine,
            weapon_kind,
            toss_grenade: self.controller.toss_grenade,
            change_weapon: self.weapon_change_direction != RequiredWeapon::None,
            scene,
        });

        self.state_machine.handle_animation_events(
            &self.character,
            sound_manager,
            self.position(&scene.graph),
            scene,
            is_walking,
            self.run_factor,
            has_ground_contact,
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
            let aiming = self
                .state_machine
                .upper_body_layer(&scene.graph)
                .map(|l| l.active_state() == self.state_machine.aim_state)
                .unwrap_or(false);

            if aiming {
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

    fn can_move(&self, graph: &Graph) -> bool {
        if let Some(layer) = graph
            .try_get_of_type::<AnimationBlendingStateMachine>(self.machine)
            .and_then(|absm| absm.machine().layers().first())
        {
            layer.active_state() != self.state_machine.fall_state
                && layer.active_state() != self.state_machine.land_state
        } else {
            true
        }
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
            && !self.state_machine.is_stunned(scene, self.animation_player)
    }

    pub fn is_aiming(&self) -> bool {
        self.controller.aim
    }

    pub fn is_completely_dead(&self, scene: &Scene) -> bool {
        let animations_container =
            utils::fetch_animation_container_ref(&scene.graph, self.animation_player);
        self.is_dead()
            && (animations_container[self.state_machine.dying_animation].has_ended()
                || animations_container[self.state_machine.dying_animation].has_ended())
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
        self.item_display = SpriteBuilder::new(BaseBuilder::new().with_depth_offset(0.05))
            .with_size(0.1)
            .build(&mut context.scene.graph);

        // Add default weapon.
        self.push_command(CharacterCommand::AddWeapon(WeaponKind::Glock));
        self.push_command(CharacterCommand::AddWeapon(WeaponKind::M4));

        self.inventory.add_item(ItemKind::Grenade, 10);

        let level = current_level_mut(context.plugins).unwrap();

        level.actors.push(context.handle);
        // Also register player in special variable to speed up access.
        level.player = context.handle;
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        let game = game_ref(ctx.plugins);

        self.state_machine = StateMachine::new(self.machine, &ctx.scene.graph).unwrap();

        self.resolve(
            ctx.scene,
            game.weapon_display.render_target.clone(),
            game.inventory_interface.render_target.clone(),
            game.item_display.render_target.clone(),
            game.journal_display.render_target.clone(),
        );
    }

    fn on_deinit(&mut self, context: &mut ScriptDeinitContext) {
        if let Some(level) = current_level_mut(context.plugins) {
            level.player = Handle::NONE;

            if let Some(position) = level.actors.iter().position(|a| *a == context.node_handle) {
                level.actors.remove(position);
            }
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
                        .clamp(-90.0f32.to_radians(), 90.0f32.to_radians());
                    None
                }
                _ => None,
            },
            _ => None,
        };

        let animations_container =
            utils::fetch_animation_container_mut(&mut context.scene.graph, self.animation_player);

        let jump_anim = animations_container.get(self.state_machine.jump_animation);
        let can_jump = !jump_anim.is_enabled() || jump_anim.has_ended();

        let can_change_weapon = self.weapon_change_direction.is_none()
            && animations_container[self.state_machine.grab_animation].has_ended()
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
                if state == ElementState::Pressed && can_jump {
                    let animations_container = utils::fetch_animation_container_mut(
                        &mut context.scene.graph,
                        self.animation_player,
                    );

                    // Rewind jump animation to beginning before jump.
                    animations_container
                        .get_mut(self.state_machine.jump_animation)
                        .set_enabled(true)
                        .rewind();
                    animations_container
                        .get_mut(self.state_machine.jump_animation)
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
                        let animations_container = utils::fetch_animation_container_mut(
                            &mut context.scene.graph,
                            self.animation_player,
                        );

                        animations_container
                            .get_mut(self.state_machine.toss_grenade_animation)
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

            let animations_container = utils::fetch_animation_container_mut(
                &mut context.scene.graph,
                self.animation_player,
            );

            animations_container
                .get_mut(self.state_machine.put_back_animation)
                .rewind();

            animations_container
                .get_mut(self.state_machine.grab_animation)
                .set_enabled(false)
                .rewind();
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = game_mut(ctx.plugins);
        game.weapon_display.sync_to_model(self, &ctx.scene.graph);
        game.journal_display.update(ctx.dt, &self.journal);

        let game = game_ref(ctx.plugins);
        let level = current_level_ref(ctx.plugins).unwrap();

        while self
            .poll_command(
                ctx.scene,
                ctx.handle,
                ctx.resource_manager,
                &level.sound_manager,
            )
            .is_some()
        {
            // TODO: Handle commands here
        }

        self.update_health_cylinder(ctx.scene);

        let has_ground_contact = self.has_ground_contact(&ctx.scene.graph);
        let is_walking = self.is_walking();
        let is_jumping = has_ground_contact && self.controller.jump;

        self.update_animation_machines(
            ctx.scene,
            is_walking,
            is_jumping,
            has_ground_contact,
            &level.sound_manager,
        );

        let is_running = self.is_running(ctx.scene);

        if !self.is_dead() {
            if is_running {
                self.target_run_factor = 1.0;
            } else {
                self.target_run_factor = 0.0;
            }
            self.run_factor += (self.target_run_factor - self.run_factor) * 0.1;

            let can_move = self.can_move(&ctx.scene.graph);
            self.update_velocity(ctx.scene, can_move, ctx.dt);
            let new_y_vel = self.handle_jump_signal(ctx.scene, ctx.dt);
            self.handle_weapon_grab_signal(ctx.scene);
            self.handle_put_back_weapon_end_signal(ctx.scene);
            self.handle_toss_grenade_signal(Default::default(), ctx.scene, ctx.resource_manager);

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
                    self.state_machine.walk_animation,
                    self.state_machine.walk_animation,
                    self.state_machine.run_animation,
                    self.state_machine.run_animation,
                ] {
                    let animations_container = utils::fetch_animation_container_mut(
                        &mut ctx.scene.graph,
                        self.animation_player,
                    );
                    animations_container.get_mut(animation).set_speed(walk_dir);
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
                    self.state_machine.land_animation,
                    self.state_machine.land_animation,
                ] {
                    let animations_container = utils::fetch_animation_container_mut(
                        &mut ctx.scene.graph,
                        self.animation_player,
                    );
                    animations_container.get_mut(land_animation).rewind();
                }
            }

            ctx.scene.graph[self.item_display].set_visibility(false);

            self.check_doors(ctx.scene, &level.doors_container);
            self.check_elevators(ctx.scene, &level.elevators);
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
                self.state_machine.dying_animation,
                self.state_machine.dying_animation,
            ] {
                let animations_container = utils::fetch_animation_container_mut(
                    &mut ctx.scene.graph,
                    self.animation_player,
                );
                animations_container
                    .get_mut(dying_animation)
                    .set_enabled(true);
            }

            // Lock player on the place he died.
            let body = ctx.scene.graph[self.body].as_rigid_body_mut();
            body.set_ang_vel(Default::default());
            body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));

            if self.is_completely_dead(ctx.scene) {
                game.message_sender.send(Message::EndMatch);
            }
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
