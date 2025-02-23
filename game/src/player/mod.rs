use crate::character::DamagePosition;
use crate::{
    character::{Character, CharacterMessage, CharacterMessageData, DamageDealer},
    control_scheme::ControlButton,
    door::{door_mut, DoorContainer},
    elevator::call_button::{CallButton, CallButtonKind},
    gui::journal::Journal,
    inventory::Inventory,
    level::hit_box::HitBoxMessage,
    level::item::ItemAction,
    message::Message,
    player::state_machine::{StateMachine, StateMachineInput},
    sound::SoundManager,
    utils::{self},
    weapon::{
        projectile::Projectile, weapon_ref, CombatWeaponKind, Weapon, WeaponMessage,
        WeaponMessageData,
    },
    CameraController, Elevator, Game, Item, MessageSender,
};
use fyrox::fxhash::FxHashSet;
use fyrox::{
    asset::manager::ResourceManager,
    core::{
        algebra::{UnitQuaternion, Vector2, Vector3},
        color::Color,
        color_gradient::{ColorGradient, ColorGradientBuilder, GradientPoint},
        futures::executor::block_on,
        log::Log,
        math::{SmoothAngle, Vector2Ext},
        pool::Handle,
        reflect::prelude::*,
        type_traits::prelude::*,
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    event::{DeviceEvent, ElementState, Event, MouseScrollDelta, WindowEvent},
    graph::{BaseSceneGraph, SceneGraph, SceneGraphNode},
    keyboard::PhysicalKey,
    resource::{
        model::{Model, ModelResource, ModelResourceExtension},
        texture::TextureResource,
    },
    scene::{
        animation::{absm, absm::prelude::*, prelude::*},
        collider::Collider,
        graph::Graph,
        light::BaseLight,
        node::Node,
        sprite::Sprite,
        Scene,
    },
    script::RoutingStrategy,
    script::{
        PluginsRefMut, ScriptContext, ScriptDeinitContext, ScriptMessageContext,
        ScriptMessagePayload, ScriptMessageSender, ScriptTrait,
    },
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

#[derive(Clone, PartialEq, Eq, Visit, Debug)]
pub enum RequiredWeapon {
    None,
    Next,
    Previous,
    Specific(ModelResource),
}

impl RequiredWeapon {
    fn is_none(&self) -> bool {
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
    pub current_weapon: usize,
    pub weapons: Vec<ModelResource>,
}

#[derive(Default, Clone, Debug)]
struct MeleeAttackContext {
    damaged_enemies: FxHashSet<Handle<Node>>,
}

#[derive(Visit, Reflect, Debug, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "50a07510-893d-476f-aad2-fcfb0845807f")]
#[visit(optional)]
pub struct Player {
    #[component(include)]
    character: Character,
    pub camera_controller: Handle<Node>,
    model_pivot: Handle<Node>,
    model_sub_pivot: Handle<Node>,
    model: Handle<Node>,
    model_yaw: SmoothAngle,
    yaw: InheritableVariable<SmoothAngle>,
    spine_pitch: SmoothAngle,
    spine: Handle<Node>,
    hips: Handle<Node>,
    weapon_yaw_correction: SmoothAngle,
    weapon_pitch_correction: SmoothAngle,
    run_factor: f32,
    target_run_factor: f32,
    in_air_time: f32,
    velocity: Vector3<f32>,
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
    local_velocity: Vector2<f32>,
    target_local_velocity: Vector2<f32>,
    flash_light: InheritableVariable<Handle<Node>>,
    flash_light_enabled: InheritableVariable<bool>,
    #[reflect(min_value = 0.0, max_value = 20.0)]
    melee_attack_damage: InheritableVariable<f32>,
    ak47_weapon: Option<ModelResource>,
    m4_weapon: Option<ModelResource>,
    glock_weapon: Option<ModelResource>,
    plasma_gun_weapon: Option<ModelResource>,
    melee_hit_box: InheritableVariable<Handle<Node>>,
    animation_player: Handle<Node>,
    target_yaw: f32,
    target_pitch: f32,

    item_display_prefab: Option<ModelResource>,
    #[reflect(hidden)]
    item_display: Handle<Node>,

    #[reflect(hidden)]
    #[visit(skip)]
    state_machine: StateMachine,

    #[reflect(hidden)]
    weapon_change_direction: RequiredWeapon,

    #[reflect(hidden)]
    #[visit(skip)]
    melee_attack_context: Option<MeleeAttackContext>,

    #[reflect(hidden)]
    pub journal: Journal,

    #[visit(skip)]
    #[reflect(hidden)]
    controller: InputController,

    #[visit(skip)]
    #[reflect(hidden)]
    pub script_message_sender: Option<ScriptMessageSender>,
    pub grenade_item: InheritableVariable<Option<ModelResource>>,
}

impl Default for Player {
    fn default() -> Self {
        let angular_speed = 570.0f32.to_radians();
        Self {
            character: Default::default(),
            rig_light: Default::default(),
            camera_controller: Default::default(),
            inventory_display: Default::default(),
            model: Default::default(),
            controller: Default::default(),
            health_cylinder: Default::default(),
            spine: Default::default(),
            hips: Default::default(),
            model_yaw: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: angular_speed,
            },
            yaw: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: angular_speed,
            }
            .into(),
            spine_pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: angular_speed,
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
            target_yaw: 0.0,
            machine: Default::default(),
            local_velocity: Default::default(),
            state_machine: Default::default(),
            script_message_sender: None,
            target_local_velocity: Default::default(),
            flash_light: Default::default(),
            flash_light_enabled: true.into(),
            melee_attack_damage: 7.5.into(),
            ak47_weapon: None,
            m4_weapon: None,
            glock_weapon: None,
            plasma_gun_weapon: None,
            grenade_item: Default::default(),
            melee_hit_box: Default::default(),
            melee_attack_context: Default::default(),
            target_pitch: 0.0,
            item_display_prefab: None,
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
            yaw: self.yaw.clone(),
            spine_pitch: self.spine_pitch.clone(),
            spine: self.spine,
            hips: self.hips,
            weapon_yaw_correction: self.weapon_yaw_correction.clone(),
            weapon_pitch_correction: self.weapon_pitch_correction.clone(),
            run_factor: self.run_factor,
            target_run_factor: self.target_run_factor,
            in_air_time: self.in_air_time,
            velocity: self.velocity,
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
            weapon_change_direction: self.weapon_change_direction.clone(),
            melee_attack_context: self.melee_attack_context.clone(),
            journal: Default::default(),
            controller: Default::default(),
            animation_player: self.animation_player,
            target_yaw: self.target_yaw,
            machine: self.machine,
            local_velocity: self.local_velocity,
            state_machine: self.state_machine.clone(),
            script_message_sender: self.script_message_sender.clone(),
            target_local_velocity: self.target_local_velocity,
            flash_light: self.flash_light.clone(),
            flash_light_enabled: self.flash_light_enabled.clone(),
            melee_attack_damage: self.melee_attack_damage.clone(),
            ak47_weapon: self.ak47_weapon.clone(),
            m4_weapon: self.m4_weapon.clone(),
            glock_weapon: self.glock_weapon.clone(),
            plasma_gun_weapon: self.plasma_gun_weapon.clone(),
            grenade_item: self.grenade_item.clone(),
            melee_hit_box: self.melee_hit_box.clone(),
            target_pitch: self.target_pitch,
            item_display_prefab: self.item_display_prefab.clone(),
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
    pub fn persistent_data(&self, graph: &Graph) -> PlayerPersistentData {
        PlayerPersistentData {
            inventory: self.inventory.clone(),
            health: self.health,
            current_weapon: self.current_weapon,
            weapons: self
                .weapons
                .iter()
                .filter_map(|w| graph[*w].root_resource())
                .collect::<Vec<_>>(),
        }
    }

    fn check_items(
        &mut self,
        game: &mut Game,
        scene: &mut Scene,
        self_handle: Handle<Node>,
        script_message_sender: &ScriptMessageSender,
    ) {
        let items = &game.level.as_ref().unwrap().items;
        for &item_handle in items.iter() {
            if let Some(item_node) = scene.graph.try_get(item_handle) {
                if !item_node.is_globally_enabled() {
                    continue;
                }

                let item = item_node.try_get_script_component::<Item>().unwrap();

                if !item.enabled {
                    continue;
                }

                let self_position = scene.graph[self.body].global_position();
                let item_position = item_node.global_position();

                let distance = (item_position - self_position).norm();
                if distance < 0.75 {
                    if let Some(resource) = item_node.root_resource() {
                        game.item_display.sync_to_model(
                            resource,
                            *item.stack_size,
                            &game.config.controls,
                        );
                    }

                    if self.controller.action {
                        script_message_sender.send_to_target(
                            self_handle,
                            CharacterMessage {
                                character: self_handle,
                                data: CharacterMessageData::PickupItem(item_handle),
                            },
                        );

                        self.controller.action = false;
                    }

                    if let Some(display) = scene.graph.try_get_mut(self.item_display) {
                        display
                            .local_transform_mut()
                            .set_position(item_position + Vector3::new(0.0, 0.2, 0.0));
                        display.set_visibility(true);
                    }

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
                    door.try_open(Some(&self.inventory));
                }
            }
        }
    }

    fn check_elevators(&self, scene: &mut Scene, elevators: &[Handle<Node>]) {
        let graph = &mut scene.graph;
        let self_position = graph[self.body].global_position();

        for &elevator_handle in elevators.iter() {
            let mbc = graph.begin_multi_borrow();

            let mut elevator_node = mbc.try_get_mut(elevator_handle).unwrap();
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
                if let Ok(mut call_button_node) = mbc.try_get_mut(call_button_handle) {
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
                    Log::warn(format!("Unable to get call button {call_button_handle:?}!"));
                }
            }

            if let Some(requested_floor) = requested_floor {
                elevator_script.call_to(requested_floor);
            }
        }
    }

    fn handle_animation_signals(
        &mut self,
        scene: &mut Scene,
        game_message_sender: &MessageSender,
        script_message_sender: &ScriptMessageSender,
        self_handle: Handle<Node>,
        resource_manager: &ResourceManager,
        position: Vector3<f32>,
        is_walking: bool,
        has_ground_contact: bool,
        sound_manager: &SoundManager,
    ) {
        if let Some(absm) = scene
            .graph
            .try_get_of_type::<AnimationBlendingStateMachine>(self.machine)
        {
            let animation_player_handle = absm.animation_player();

            if let Some(animation_player) = scene
                .graph
                .try_get_of_type::<AnimationPlayer>(animation_player_handle)
            {
                let upper_layer_events = self
                    .state_machine
                    .upper_body_layer(&scene.graph)
                    .map(|l| {
                        l.collect_active_animations_events(
                            absm.machine().parameters(),
                            animation_player.animations(),
                            AnimationEventCollectionStrategy::All,
                        )
                    })
                    .unwrap_or_default();

                let lower_layer_max_weight_events = self
                    .state_machine
                    .lower_body_layer(&scene.graph)
                    .map(|l| {
                        l.collect_active_animations_events(
                            absm.machine().parameters(),
                            animation_player.animations(),
                            AnimationEventCollectionStrategy::MaxWeight,
                        )
                    })
                    .unwrap_or_default();

                let lower_layer_all_events = self
                    .state_machine
                    .lower_body_layer(&scene.graph)
                    .map(|l| {
                        l.collect_active_animations_events(
                            absm.machine().parameters(),
                            animation_player.animations(),
                            AnimationEventCollectionStrategy::All,
                        )
                    })
                    .unwrap_or_default();

                for (_, event) in upper_layer_events.events {
                    if event.name == StateMachine::GRAB_WEAPON_SIGNAL {
                        match &self.weapon_change_direction {
                            RequiredWeapon::None => (),
                            RequiredWeapon::Next => self.next_weapon(&mut scene.graph),
                            RequiredWeapon::Previous => self.prev_weapon(&mut scene.graph),
                            RequiredWeapon::Specific(weapon_resource) => {
                                script_message_sender.send_to_target(
                                    self_handle,
                                    CharacterMessage {
                                        character: self_handle,
                                        data: CharacterMessageData::SelectWeapon(
                                            weapon_resource.clone(),
                                        ),
                                    },
                                );
                            }
                        }

                        self.weapon_change_direction = RequiredWeapon::None;
                    } else if event.name == StateMachine::TOSS_GRENADE_SIGNAL {
                        let position = scene.graph[self.weapon_pivot].global_position();

                        let direction = scene
                            .graph
                            .try_get(self.camera_controller)
                            .and_then(|c| c.try_get_script::<CameraController>())
                            .map(|c| scene.graph[c.camera()].look_vector())
                            .unwrap_or_default();

                        if let Some(grenade_item) = self.grenade_item.deref().clone() {
                            if self.inventory.try_extract_exact_items(&grenade_item, 1) == 1 {
                                if let Ok(grenade) = block_on(
                                    resource_manager
                                        .request::<Model>("data/models/grenade/grenade_proj.rgs"),
                                ) {
                                    Projectile::spawn(
                                        &grenade,
                                        scene,
                                        direction,
                                        position,
                                        self_handle,
                                        direction.scale(10.0),
                                    );
                                }
                            }
                        }
                    } else if event.name == StateMachine::HIT_STARTED_SIGNAL {
                        self.melee_attack_context = Some(Default::default());
                    } else if event.name == StateMachine::HIT_ENDED_SIGNAL {
                        self.melee_attack_context = None;
                    }
                }

                for (_, event) in lower_layer_max_weight_events.events {
                    if event.name == StateMachine::FOOTSTEP_SIGNAL {
                        let begin = position + Vector3::new(0.0, 0.5, 0.0);

                        if is_walking && has_ground_contact {
                            self.character
                                .footstep_ray_check(begin, scene, sound_manager);
                        }
                    }
                }

                for (_, event) in lower_layer_all_events.events {
                    if event.name == "Died" {
                        game_message_sender.send(Message::EndMatch);
                    }
                }

                // Clear all the events.
                scene
                    .graph
                    .try_get_mut_of_type::<AnimationPlayer>(animation_player_handle)
                    .unwrap()
                    .animations_mut()
                    .get_value_mut_silent()
                    .clear_animation_events();
            }
        }
    }

    fn update_velocity(&mut self, scene: &mut Scene, dt: f32) {
        let transform = &scene.graph[self.model].global_transform();

        if let Some(root_motion) = self
            .state_machine
            .lower_body_layer(&scene.graph)
            .unwrap()
            .pose()
            .root_motion()
        {
            self.velocity = transform
                .transform_vector(&root_motion.delta_position)
                .scale(1.0 / dt);
        }

        let body = scene.graph[self.body].as_rigid_body_mut();

        body.set_ang_vel(Default::default());
        body.set_lin_vel(Vector3::new(
            self.velocity.x,
            if self.velocity.y > 0.001 {
                self.velocity.y
            } else {
                body.lin_vel().y
            },
            self.velocity.z,
        ));
    }

    fn current_weapon_kind(&self, graph: &Graph) -> CombatWeaponKind {
        if let Some(current_weapon) = graph.try_get_script_of::<Weapon>(self.current_weapon()) {
            current_weapon.weapon_type
        } else {
            CombatWeaponKind::Pistol
        }
    }

    fn should_be_stunned(&self) -> bool {
        self.last_health - self.health >= 15.0
    }

    fn stun(&mut self) {
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
        let color = self
            .health_color_gradient
            .get_color(self.health / *self.max_health);
        let surface = mesh.surfaces_mut().first_mut().unwrap();
        let mut material = surface.material().data_ref();
        material.set_property("diffuseColor", color);
        material.set_property("emissionStrength", color.as_frgb().scale(10.0));
        drop(material);
        scene.graph[self.rig_light]
            .component_mut::<BaseLight>()
            .unwrap()
            .set_color(color);
    }

    fn update_animation_machines(&mut self, scene: &mut Scene, is_walking: bool, is_jumping: bool) {
        let weapon_kind = self.current_weapon_kind(&scene.graph);

        let should_be_stunned = self.should_be_stunned();
        if should_be_stunned {
            self.stun();
        }

        self.state_machine.apply(StateMachineInput {
            is_walking,
            is_jumping,
            has_ground_contact: self.in_air_time <= 0.3,
            is_aiming: self.controller.aim && !self.character.weapons.is_empty(),
            run_factor: self.run_factor,
            is_dead: self.is_dead(),
            should_be_stunned,
            melee_attack: self.controller.shoot && !self.controller.aim,
            machine: self.machine,
            weapon_kind,
            toss_grenade: self.controller.toss_grenade,
            change_weapon: self.weapon_change_direction != RequiredWeapon::None,
            scene,
            local_velocity: self.local_velocity,
            hit_something: self
                .melee_attack_context
                .as_ref()
                .map(|ctx| !ctx.damaged_enemies.is_empty())
                .unwrap_or_default(),
        });
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

    fn update_shooting(
        &mut self,
        scene: &mut Scene,
        dt: f32,
        elapsed_time: f32,
        script_message_sender: &ScriptMessageSender,
    ) {
        self.v_recoil.update(dt);
        self.h_recoil.update(dt);

        if let Some(&current_weapon_handle) =
            self.character.weapons.get(self.character.current_weapon)
        {
            let aiming = self
                .state_machine
                .upper_body_layer(&scene.graph)
                .map(|l| l.active_state() == self.state_machine.aim_state)
                .unwrap_or(false);

            if aiming {
                let ammo_indicator_offset =
                    *weapon_ref(current_weapon_handle, &scene.graph).ammo_indicator_offset;
                let weapon_display = &mut scene.graph[self.weapon_display];
                weapon_display.set_visibility(true);
                weapon_display
                    .local_transform_mut()
                    .set_position(ammo_indicator_offset);

                let current_weapon = weapon_ref(current_weapon_handle, &scene.graph);
                if self.controller.shoot && current_weapon.can_shoot(elapsed_time) {
                    let ammo_per_shot = *current_weapon.ammo_consumption_per_shot;

                    // A weapon could have infinite ammo, in this case ammo item is not specified.
                    let enough_ammo = current_weapon.ammo_item.as_ref().map_or(true, |ammo_item| {
                        self.inventory
                            .try_extract_exact_items(ammo_item, ammo_per_shot)
                            == ammo_per_shot
                    });

                    if enough_ammo {
                        script_message_sender.send_to_target(
                            current_weapon_handle,
                            WeaponMessage {
                                weapon: current_weapon_handle,
                                data: WeaponMessageData::Shoot {
                                    direction: Default::default(),
                                },
                            },
                        );

                        if *current_weapon.shake_camera_on_shot {
                            self.v_recoil
                                .set_target(current_weapon.gen_v_recoil_angle());
                            self.h_recoil
                                .set_target(current_weapon.gen_h_recoil_angle());

                            if let Some(camera_controller) = scene
                                .graph
                                .try_get_mut(self.camera_controller)
                                .and_then(|c| c.try_get_script_mut::<CameraController>())
                            {
                                camera_controller.request_shake_camera();
                            }
                        }
                    }
                }
            } else {
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
            let (pitch_correction, yaw_correction) = if let Some(weapon) = scene
                .graph
                .try_get_script_of::<Weapon>(self.current_weapon())
            {
                (*weapon.pitch_correction, *weapon.yaw_correction)
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

    fn update_melee_attack(
        &mut self,
        scene: &mut Scene,
        script_message_sender: &ScriptMessageSender,
        self_handle: Handle<Node>,
        plugins: &PluginsRefMut,
    ) -> Option<()> {
        let attack_context = self.melee_attack_context.as_mut()?;
        let melee_hit_box_collider = scene
            .graph
            .try_get_of_type::<Collider>(*self.melee_hit_box)?;
        let level = plugins.get::<Game>().level.as_ref()?;

        for intersection in melee_hit_box_collider.intersects(&scene.graph.physics) {
            for &hit_box in level.hit_boxes.iter() {
                if hit_box == self.character.capsule_collider {
                    continue;
                }

                if self.character.hit_boxes.contains(&hit_box) {
                    continue;
                }

                if hit_box == intersection.collider1 || hit_box == intersection.collider2 {
                    if attack_context.damaged_enemies.contains(&hit_box) {
                        continue;
                    }
                    attack_context.damaged_enemies.insert(hit_box);

                    script_message_sender.send_hierarchical(
                        hit_box,
                        RoutingStrategy::Up,
                        HitBoxMessage {
                            hit_box,
                            damage: *self.melee_attack_damage,
                            dealer: DamageDealer {
                                entity: self_handle,
                            },
                            position: Some(DamagePosition {
                                point: melee_hit_box_collider.global_position(),
                                direction: Vector3::new(0.0, 0.0, 1.0),
                            }),
                            is_melee: true,
                        },
                    );
                }
            }
        }

        None
    }

    pub fn resolve(
        &mut self,
        scene: &mut Scene,
        display_texture: TextureResource,
        inventory_texture: TextureResource,
        item_texture: TextureResource,
        journal_texture: TextureResource,
    ) {
        scene.graph[self.weapon_display]
            .as_mesh_mut()
            .surfaces_mut()
            .first_mut()
            .unwrap()
            .material()
            .data_ref()
            .bind("diffuseTexture", display_texture);

        scene.graph[self.inventory_display]
            .as_mesh_mut()
            .surfaces_mut()
            .first_mut()
            .unwrap()
            .material()
            .data_ref()
            .bind("diffuseTexture", inventory_texture);

        scene.graph[self.journal_display]
            .as_mesh_mut()
            .surfaces_mut()
            .first_mut()
            .unwrap()
            .material()
            .data_ref()
            .bind("diffuseTexture", journal_texture);

        if let Some(item_display) = scene.graph.try_get_of_type::<Sprite>(self.item_display) {
            item_display
                .material()
                .data_ref()
                .bind("diffuseTexture", item_texture);
        }

        self.health_color_gradient = make_color_gradient();
    }
}

impl ScriptTrait for Player {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        if let Some(item_display_prefab) = self.item_display_prefab.as_ref() {
            self.item_display = item_display_prefab.instantiate(ctx.scene);
        }

        if let Some(grenade_item) = self.grenade_item.deref().clone() {
            self.inventory.add_item(&grenade_item, 10);
        }

        let level = ctx.plugins.get_mut::<Game>().level.as_mut().unwrap();

        level.actors.push(ctx.handle);
        // Also register player in special variable to speed up access.
        level.player = ctx.handle;
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.character.on_start(ctx);

        let game = ctx.plugins.get::<Game>();

        ctx.message_dispatcher
            .subscribe_to::<CharacterMessage>(ctx.handle);
        ctx.message_dispatcher
            .subscribe_to::<WeaponMessage>(ctx.handle);
        ctx.message_dispatcher
            .subscribe_to::<HitBoxMessage>(ctx.handle);

        self.script_message_sender = Some(ctx.message_sender.clone());

        self.state_machine = StateMachine::new(self.machine, &ctx.scene.graph).unwrap();

        self.resolve(
            ctx.scene,
            game.weapon_display.render_target.clone(),
            game.inventory_interface.render_target.clone(),
            game.item_display.render_target.clone(),
            game.journal_display.render_target.clone(),
        );
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        if let Some(level) = ctx.plugins.get_mut::<Game>().level.as_mut() {
            level.player = Handle::NONE;

            if let Some(position) = level.actors.iter().position(|a| *a == ctx.node_handle) {
                level.actors.remove(position);
            }
        }
    }

    fn on_os_event(&mut self, event: &Event<()>, ctx: &mut ScriptContext) {
        let game = ctx.plugins.get::<Game>();
        let control_scheme = &game.config.controls;
        let sender = &game.message_sender;

        let button_state = match event {
            Event::WindowEvent { event, .. } => {
                if let WindowEvent::KeyboardInput { event: input, .. } = event {
                    if let PhysicalKey::Code(key) = input.physical_key {
                        Some((ControlButton::Key(key), input.state))
                    } else {
                        None
                    }
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
                    let mouse_sens = control_scheme.mouse_sens * ctx.dt;
                    self.target_yaw -= (delta.0 as f32) * mouse_sens;
                    let pitch_direction = if control_scheme.mouse_y_inverse {
                        -1.0
                    } else {
                        1.0
                    };
                    self.target_pitch = (self.target_pitch
                        + pitch_direction * (delta.1 as f32) * mouse_sens)
                        .clamp(-90.0f32.to_radians(), 90.0f32.to_radians());
                    None
                }
                _ => None,
            },
            _ => None,
        };

        let animations_container =
            utils::fetch_animation_container_mut(&mut ctx.scene.graph, self.animation_player);

        let jump_anim = animations_container.get(self.state_machine.jump_animation);
        let can_jump = !jump_anim.is_enabled() || jump_anim.has_ended();

        let can_change_weapon = self.weapon_change_direction.is_none()
            && animations_container[self.state_machine.grab_animation].has_ended()
            && self.weapons.len() > 1;

        let current_weapon_kind = ctx
            .scene
            .graph
            .try_get(self.current_weapon())
            .and_then(|node| node.root_resource());

        let mut weapon_change_direction = None;

        if let Some((button, state)) = button_state {
            if button == control_scheme.aim.button {
                self.controller.aim = state == ElementState::Pressed;
                if state == ElementState::Pressed {
                    ctx.scene.graph[self.inventory_display].set_visibility(false);
                    ctx.scene.graph[self.journal_display].set_visibility(false);
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
                self.controller.jump = state == ElementState::Pressed && can_jump;
            } else if button == control_scheme.run.button {
                self.controller.run = state == ElementState::Pressed;
            } else if button == control_scheme.flash_light.button {
                if state == ElementState::Pressed {
                    let enabled = *self.flash_light_enabled;
                    self.flash_light_enabled
                        .set_value_and_mark_modified(!enabled);
                }
            } else if button == control_scheme.grab_ak47.button && can_change_weapon {
                if current_weapon_kind != self.ak47_weapon {
                    if let Some(ak47_weapon) = self.ak47_weapon.clone() {
                        weapon_change_direction = Some(RequiredWeapon::Specific(ak47_weapon));
                    }
                }
            } else if button == control_scheme.grab_m4.button && can_change_weapon {
                if current_weapon_kind != self.m4_weapon {
                    if let Some(m4_weapon) = self.m4_weapon.clone() {
                        weapon_change_direction = Some(RequiredWeapon::Specific(m4_weapon));
                    }
                }
            } else if button == control_scheme.grab_plasma_gun.button && can_change_weapon {
                if current_weapon_kind != self.plasma_gun_weapon {
                    if let Some(plasma_gun_weapon) = self.plasma_gun_weapon.clone() {
                        weapon_change_direction = Some(RequiredWeapon::Specific(plasma_gun_weapon));
                    }
                }
            } else if button == control_scheme.grab_pistol.button && can_change_weapon {
                if current_weapon_kind != self.glock_weapon {
                    if let Some(glock_weapon) = self.glock_weapon.clone() {
                        weapon_change_direction = Some(RequiredWeapon::Specific(glock_weapon));
                    }
                }
            } else if button == control_scheme.next_weapon.button {
                if state == ElementState::Pressed
                    && self.current_weapon < self.weapons.len().saturating_sub(1)
                    && can_change_weapon
                {
                    weapon_change_direction = Some(RequiredWeapon::Next);
                }
            } else if button == control_scheme.prev_weapon.button {
                if state == ElementState::Pressed && self.current_weapon > 0 && can_change_weapon {
                    weapon_change_direction = Some(RequiredWeapon::Previous);
                }
            } else if button == control_scheme.toss_grenade.button {
                if let Some(grenade_item) = self.grenade_item.as_ref() {
                    if self.inventory.item_count(grenade_item) > 0 {
                        self.controller.toss_grenade = state == ElementState::Pressed;
                    }
                }
            } else if button == control_scheme.quick_heal.button {
                if state == ElementState::Pressed && self.health < *self.max_health {
                    let mut min_health = f32::MAX;
                    let mut suitable_item = None;
                    for item in self.inventory.items() {
                        if let Some(resource) = item.resource.as_ref() {
                            Item::from_resource(resource, |item| {
                                if let Some(item_ref) = item {
                                    if let ItemAction::Heal { amount } = *item_ref.action {
                                        if amount < min_health {
                                            min_health = amount;
                                            suitable_item = Some(resource.clone());
                                        }
                                    }
                                }
                            });
                        }
                    }
                    if let Some(suitable_item) = suitable_item {
                        if self
                            .inventory_mut()
                            .try_extract_exact_items(&suitable_item, 1)
                            == 1
                        {
                            Item::from_resource(&suitable_item, |item| {
                                self.use_item(item.unwrap());
                            })
                        }
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
                ctx.scene.graph[self.journal_display].set_visibility(false);

                let inventory = &mut ctx.scene.graph[self.inventory_display];
                let new_visibility = !inventory.visibility();
                inventory.set_visibility(new_visibility);
            } else if button == control_scheme.journal.button
                && state == ElementState::Pressed
                && !self.controller.aim
            {
                ctx.scene.graph[self.inventory_display].set_visibility(false);

                let journal = &mut ctx.scene.graph[self.journal_display];
                let new_visibility = !journal.visibility();
                journal.set_visibility(new_visibility);
                if new_visibility {
                    sender.send(Message::SyncJournal);
                }
            }
        }

        if let Some(weapon_change_direction) = weapon_change_direction {
            self.weapon_change_direction = weapon_change_direction;
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

            let level = ctx.plugins.get::<Game>().level.as_ref().unwrap();

            self.character.on_character_message(
                &char_message.data,
                ctx.scene,
                ctx.handle,
                ctx.message_sender,
                &level.sound_manager,
            );
        } else if let Some(weapon_message) = message.downcast_ref() {
            self.character
                .on_weapon_message(weapon_message, &mut ctx.scene.graph);
        } else if let Some(hit_box_message) = message.downcast_ref::<HitBoxMessage>() {
            self.character.on_hit_box_message(hit_box_message);
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let game = ctx.plugins.get_mut::<Game>();
        game.weapon_display.sync_to_model(self, &ctx.scene.graph);
        game.journal_display.update(ctx.dt, &self.journal);

        let game = ctx.plugins.get::<Game>();
        let level = game.level.as_ref().unwrap();

        self.target_local_velocity = Vector2::default();
        if self.controller.walk_forward
            || (!self.controller.aim && (self.controller.walk_left || self.controller.walk_right))
        {
            self.target_local_velocity.y = if self.controller.run && !self.controller.aim {
                1.0
            } else {
                0.5
            };
        }
        if self.controller.walk_backward {
            self.target_local_velocity.y = if self.controller.aim {
                -1.0
            } else if self.controller.run {
                1.0
            } else {
                0.5
            };
        }
        if self.controller.aim {
            if self.controller.walk_left {
                self.target_local_velocity.x = -1.0;
            }
            if self.controller.walk_right {
                self.target_local_velocity.x = 1.0;
            }
        }

        self.local_velocity.follow(&self.target_local_velocity, 0.1);

        if let Some(upper_body_layer) = self
            .state_machine
            .upper_body_layer_mut(&mut ctx.scene.graph)
        {
            while let Some(event) = upper_body_layer.pop_event() {
                if let absm::Event::ActiveStateChanged { prev, new } = event {
                    if prev == self.state_machine.aim_state && new != self.state_machine.aim_state {
                        ctx.message_sender.send_global(CharacterMessage {
                            character: ctx.handle,
                            data: CharacterMessageData::EndedAiming,
                        })
                    } else if prev != self.state_machine.aim_state
                        && new == self.state_machine.aim_state
                    {
                        ctx.message_sender.send_global(CharacterMessage {
                            character: ctx.handle,
                            data: CharacterMessageData::BeganAiming,
                        })
                    }
                }
            }
        }

        self.update_health_cylinder(ctx.scene);

        let has_ground_contact = self.has_ground_contact(&ctx.scene.graph);
        let is_walking = self.is_walking();
        let is_jumping = has_ground_contact && self.controller.jump;

        self.update_melee_attack(ctx.scene, ctx.message_sender, ctx.handle, &ctx.plugins);
        self.update_animation_machines(ctx.scene, is_walking, is_jumping);

        if self
            .melee_attack_context
            .as_ref()
            .map(|ctx| !ctx.damaged_enemies.is_empty())
            .unwrap_or_default()
        {
            self.melee_attack_context = None;
        }

        let is_running = self.is_running(ctx.scene);

        self.handle_animation_signals(
            ctx.scene,
            &game.message_sender,
            ctx.message_sender,
            ctx.handle,
            ctx.resource_manager,
            self.position(&ctx.scene.graph),
            is_walking,
            has_ground_contact,
            &level.sound_manager,
        );

        if !self.is_dead() {
            if is_running {
                self.target_run_factor = 1.0;
            } else {
                self.target_run_factor = 0.0;
            }
            self.run_factor += (self.target_run_factor - self.run_factor) * 0.1;

            let can_move = self.can_move(&ctx.scene.graph);
            self.update_velocity(ctx.scene, ctx.dt);

            if let Some(flash_light) = ctx.scene.graph.try_get_mut(*self.flash_light) {
                flash_light.set_visibility(*self.flash_light_enabled);
            }

            let attacking_in_direction = self.controller.aim || self.melee_attack_context.is_some();

            if attacking_in_direction {
                self.spine_pitch.set_target(self.target_pitch);
            } else {
                self.spine_pitch.set_target(0.0);
            }

            self.spine_pitch.update(ctx.dt);

            if can_move && (is_walking || attacking_in_direction) {
                self.yaw.set_target(self.target_yaw).update(ctx.dt);

                // Since we have free camera while not moving, we have to sync rotation of pivot
                // with rotation of camera so character will start moving in look direction.
                ctx.scene.graph[self.model_pivot]
                    .local_transform_mut()
                    .set_rotation(UnitQuaternion::from_axis_angle(
                        &Vector3::y_axis(),
                        self.yaw.angle,
                    ));

                // Apply additional rotation to model - it will turn in front of walking direction.
                let angle = self.calculate_model_angle();

                self.model_yaw.set_target(angle.to_radians()).update(ctx.dt);

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

                    ctx.scene.graph[self.hips]
                        .local_transform_mut()
                        .set_rotation(UnitQuaternion::from_axis_angle(
                            &Vector3::y_axis(),
                            self.model_yaw.angle,
                        ));
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
            }

            self.apply_weapon_angular_correction(ctx.scene, can_move, ctx.dt);

            if has_ground_contact {
                self.in_air_time = 0.0;
            } else {
                self.in_air_time += ctx.dt;
            }

            if let Some(item_display) = ctx.scene.graph.try_get_mut(self.item_display) {
                item_display.set_visibility(false);
            }

            self.check_doors(ctx.scene, &level.doors_container);
            self.check_elevators(ctx.scene, &level.elevators);
            self.update_shooting(ctx.scene, ctx.dt, ctx.elapsed_time, ctx.message_sender);
            self.check_items(
                ctx.plugins.get_mut::<Game>(),
                ctx.scene,
                ctx.handle,
                ctx.message_sender,
            );

            let spine_transform = ctx.scene.graph[self.spine].local_transform_mut();
            let rotation = **spine_transform.rotation();
            spine_transform.set_rotation(
                rotation
                    * UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.v_recoil.angle())
                    * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.h_recoil.angle()),
            );
        } else {
            // Lock player on the place he died.
            let body = ctx.scene.graph[self.body].as_rigid_body_mut();
            body.set_ang_vel(Default::default());
            body.set_lin_vel(Vector3::new(0.0, body.lin_vel().y, 0.0));
        }
    }
}
