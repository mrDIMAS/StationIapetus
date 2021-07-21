use crate::{
    actor::Actor,
    character::{find_hit_boxes, Character},
    control_scheme::{ControlButton, ControlScheme},
    gui::journal::Journal,
    inventory::Inventory,
    item::{ItemContainer, ItemKind},
    level::UpdateContext,
    message::Message,
    player::{
        camera::CameraController,
        lower_body::{LowerBodyMachine, LowerBodyMachineInput},
        upper_body::{CombatWeaponKind, UpperBodyMachine, UpperBodyMachineInput},
    },
    weapon::{
        projectile::{ProjectileKind, Shooter},
        WeaponContainer, WeaponKind,
    },
    CollisionGroups, GameTime,
};
use rg3d::{
    animation::{
        machine::{blend_nodes::IndexedBlendInput, Machine, PoseNode, State},
        Animation,
    },
    core::{
        algebra::{Isometry3, Matrix4, Translation3, UnitQuaternion, Vector3},
        color::Color,
        color_gradient::{ColorGradient, ColorGradientBuilder, GradientPoint},
        math::{self, SmoothAngle, UnitQuaternionExt, Vector3Ext},
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::{
        resource_manager::{MaterialSearchOptions, ResourceManager},
        ColliderHandle,
    },
    event::{DeviceEvent, ElementState, Event, MouseScrollDelta, WindowEvent},
    physics::{
        dynamics::{CoefficientCombineRule, RigidBodyBuilder},
        geometry::{ColliderBuilder, InteractionGroups},
    },
    resource::{model::Model, texture::Texture},
    scene::{
        base::BaseBuilder,
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder, RenderPath,
        },
        node::Node,
        sprite::SpriteBuilder,
        transform::TransformBuilder,
        Scene,
    },
};
use std::{
    ops::{Deref, DerefMut},
    sync::{mpsc::Sender, Arc, RwLock},
};

mod camera;
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

#[derive(Default)]
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

#[derive(Copy, Clone, PartialEq, Eq)]
enum RequiredWeapon {
    None,
    Next,
    Previous,
    Specific(WeaponKind),
}

impl RequiredWeapon {
    fn is_none(self) -> bool {
        matches!(self, RequiredWeapon::None)
    }

    fn id(self) -> u32 {
        match self {
            RequiredWeapon::None => 0,
            RequiredWeapon::Next => 1,
            RequiredWeapon::Previous => 2,
            RequiredWeapon::Specific(_) => 3,
        }
    }

    fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::None),
            1 => Ok(Self::Next),
            2 => Ok(Self::Previous),
            3 => Ok(Self::Specific(WeaponKind::default())),
            _ => Err(format!("Invalid Direction id {}!", id)),
        }
    }
}

impl Visit for RequiredWeapon {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        let mut id = self.id();
        id.visit("Id", visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(id)?;
        }

        if let RequiredWeapon::Specific(kind) = self {
            kind.visit("SpecificKind", visitor)?;
        }

        visitor.leave_region()
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

#[derive(Default)]
pub struct Player {
    character: Character,
    camera_controller: CameraController,
    model: Handle<Node>,
    controller: InputController,
    lower_body_machine: LowerBodyMachine,
    upper_body_machine: UpperBodyMachine,
    model_yaw: SmoothAngle,
    spine_pitch: SmoothAngle,
    spine: Handle<Node>,
    hips: Handle<Node>,
    move_speed: f32,
    collider: ColliderHandle,
    weapon_change_direction: RequiredWeapon,
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
    item_display: Handle<Node>,
    health_cylinder: Handle<Node>,
    last_health: f32,
    health_color_gradient: ColorGradient,
    v_recoil: SmoothAngle,
    h_recoil: SmoothAngle,
    pub journal: Journal,
}

impl Visit for Player {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.character.visit("Character", visitor)?;
        self.camera_controller.visit("CameraController", visitor)?;
        self.model.visit("Model", visitor)?;
        self.lower_body_machine.visit("LowerBodyMachine", visitor)?;
        self.upper_body_machine.visit("UpperBodyMachine", visitor)?;
        self.model_yaw.visit("ModelYaw", visitor)?;
        self.spine_pitch.visit("SpinePitch", visitor)?;
        self.hips.visit("Hips", visitor)?;
        self.spine.visit("Spine", visitor)?;
        self.move_speed.visit("MoveSpeed", visitor)?;

        self.collider.visit("Collider", visitor)?;
        self.weapon_origin.visit("WeaponOrigin", visitor)?;
        self.weapon_yaw_correction
            .visit("WeaponYawCorrection", visitor)?;
        self.weapon_pitch_correction
            .visit("WeaponPitchCorrection", visitor)?;
        self.run_factor.visit("RunFactor", visitor)?;
        self.target_run_factor.visit("TargetRunFactor", visitor)?;
        self.in_air_time.visit("InAirTime", visitor)?;
        self.velocity.visit("Velocity", visitor)?;
        self.target_velocity.visit("TargetVelocity", visitor)?;
        self.weapon_change_direction
            .visit("WeaponChangeDirection", visitor)?;
        self.weapon_display.visit("WeaponDisplay", visitor)?;
        self.inventory_display.visit("InventoryDisplay", visitor)?;
        self.journal_display.visit("JournalDisplay", visitor)?;
        self.health_cylinder.visit("HealthCylinder", visitor)?;
        self.last_health.visit("LastHealth", visitor)?;
        self.item_display.visit("ItemDisplay", visitor)?;
        self.v_recoil.visit("VRecoil", visitor)?;
        self.h_recoil.visit("HRecoil", visitor)?;
        self.journal.visit("Journal", visitor)?;

        if visitor.is_reading() {
            self.health_color_gradient = make_color_gradient();
        }

        visitor.leave_region()
    }
}

fn make_color_gradient() -> ColorGradient {
    ColorGradientBuilder::new()
        .with_point(GradientPoint::new(0.0, Color::from_rgba(255, 0, 0, 200)))
        .with_point(GradientPoint::new(1.0, Color::from_rgba(0, 255, 0, 200)))
        .build()
}

impl Player {
    pub async fn new(
        scene: &mut Scene,
        resource_manager: ResourceManager,
        position: Vector3<f32>,
        orientation: UnitQuaternion<f32>,
        sender: Sender<Message>,
        display_texture: Texture,
        inventory_texture: Texture,
        item_texture: Texture,
        journal_texture: Texture,
        persistent_data: Option<PlayerPersistentData>,
    ) -> Self {
        let body_radius = 0.2;
        let body_height = 0.25;

        let (model_resource, health_rig_resource) = rg3d::core::futures::join!(
            resource_manager.request_model(
                "data/models/agent/agent.rgs",
                MaterialSearchOptions::UsePathDirectly
            ),
            resource_manager.request_model(
                "data/models/health_rig.FBX",
                MaterialSearchOptions::RecursiveUp
            ),
        );

        let model_resource = model_resource.unwrap();

        let model_handle = model_resource.instantiate_geometry(scene);

        scene.graph[model_handle]
            .local_transform_mut()
            .set_position(Vector3::new(0.0, -body_height - body_radius, 0.0));

        let pivot = BaseBuilder::new()
            .with_children(&[model_handle])
            .build(&mut scene.graph);

        let capsule = ColliderBuilder::capsule_y(body_height, body_radius)
            .collision_groups(InteractionGroups::new(
                CollisionGroups::ActorCapsule as u32,
                0xFFFF,
            ))
            .friction_combine_rule(CoefficientCombineRule::Min)
            .friction(0.0)
            .build();
        let body = scene.physics.add_body(
            RigidBodyBuilder::new_dynamic()
                .lock_rotations()
                .position(Isometry3 {
                    translation: Translation3 { vector: position },
                    rotation: orientation,
                })
                .build(),
        );
        let collider = scene.physics.add_collider(capsule, &body);

        scene.physics_binder.bind(pivot, body);

        let locomotion_machine =
            LowerBodyMachine::new(scene, model_handle, resource_manager.clone()).await;

        let combat_machine =
            UpperBodyMachine::new(scene, model_handle, resource_manager.clone()).await;

        scene.graph.update_hierarchical_data();

        let hand = scene
            .graph
            .find_by_name(model_handle, "mixamorig:RightHand");

        let hand_scale = scene.graph.global_scale(hand);

        let weapon_pivot;
        let weapon_origin = BaseBuilder::new()
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_scale(Vector3::new(
                        1.0 / hand_scale.x,
                        1.0 / hand_scale.y,
                        1.0 / hand_scale.z,
                    ))
                    .with_local_rotation(
                        UnitQuaternion::from_axis_angle(&Vector3::x_axis(), -90.0f32.to_radians())
                            * UnitQuaternion::from_axis_angle(
                                &Vector3::z_axis(),
                                -90.0f32.to_radians(),
                            ),
                    )
                    .build(),
            )
            .with_children(&[{
                weapon_pivot = BaseBuilder::new().build(&mut scene.graph);
                weapon_pivot
            }])
            .build(&mut scene.graph);

        scene.graph.link_nodes(weapon_origin, hand);

        let health_rig = health_rig_resource.unwrap().instantiate_geometry(scene);

        let spine = scene.graph.find_by_name(model_handle, "mixamorig:Spine2");
        scene.graph.link_nodes(health_rig, spine);
        let spine_scale = scene.graph.global_scale(spine);
        scene.graph[health_rig]
            .local_transform_mut()
            .set_position(Vector3::new(0.0, -15.0, -10.5))
            .set_scale(Vector3::new(
                0.015 / spine_scale.x,
                0.015 / spine_scale.y,
                0.015 / spine_scale.z,
            ));

        let health_cylinder = scene.graph.find_by_name(health_rig, "HealthCylinder");
        let mesh = scene.graph[health_cylinder].as_mesh_mut();
        mesh.set_render_path(RenderPath::Forward);

        let weapon_display = MeshBuilder::new(BaseBuilder::new())
            .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
                SurfaceData::make_quad(&Matrix4::new_scaling(0.07)),
            )))
            .with_diffuse_texture(display_texture)
            .build()])
            .with_cast_shadows(false)
            .with_render_path(RenderPath::Forward)
            .build(&mut scene.graph);
        scene.graph.link_nodes(weapon_display, weapon_pivot);

        let item_display = SpriteBuilder::new(BaseBuilder::new().with_depth_offset(0.05))
            .with_texture(item_texture)
            .with_size(0.1)
            .build(&mut scene.graph);

        let s = 0.8;
        let inventory_display = MeshBuilder::new(
            BaseBuilder::new()
                .with_depth_offset(0.2)
                .with_visibility(false)
                .with_local_transform(
                    TransformBuilder::new()
                        .with_local_position(Vector3::new(-0.5, 0.1, 0.4))
                        .build(),
                ),
        )
        .with_cast_shadows(false)
        .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
            SurfaceData::make_quad(&Matrix4::new_nonuniform_scaling(&Vector3::new(
                s,
                3.0 / 4.0 * s,
                s,
            ))),
        )))
        .with_diffuse_texture(inventory_texture)
        .build()])
        .with_render_path(RenderPath::Forward)
        .build(&mut scene.graph);
        scene.graph.link_nodes(inventory_display, pivot);

        let journal_display = MeshBuilder::new(
            BaseBuilder::new()
                .with_depth_offset(0.2)
                .with_visibility(false)
                .with_local_transform(
                    TransformBuilder::new()
                        .with_local_position(Vector3::new(-0.5, 0.1, 0.4))
                        .build(),
                ),
        )
        .with_cast_shadows(false)
        .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
            SurfaceData::make_quad(&Matrix4::new_nonuniform_scaling(&Vector3::new(
                s,
                3.0 / 4.0 * s,
                s,
            ))),
        )))
        .with_diffuse_texture(journal_texture)
        .build()])
        .with_render_path(RenderPath::Forward)
        .build(&mut scene.graph);
        scene.graph.link_nodes(journal_display, pivot);

        let (health, inventory, current_weapon) = if let Some(persistent_data) = persistent_data {
            (
                persistent_data.health,
                persistent_data.inventory,
                persistent_data.current_weapon,
            )
        } else {
            let mut inventory = Inventory::new();

            inventory.add_item(ItemKind::Medpack, 2);
            inventory.add_item(ItemKind::Ammo, 100);
            inventory.add_item(ItemKind::Grenade, 2);

            (100.0, inventory, 0)
        };

        Self {
            character: Character {
                pivot,
                body: Some(body),
                weapon_pivot,
                sender: Some(sender),
                hit_boxes: find_hit_boxes(pivot, scene),
                health,
                current_weapon,
                inventory,
                ..Default::default()
            },
            camera_controller: CameraController::new(resource_manager.clone(), &mut scene.graph)
                .await,
            inventory_display,
            weapon_origin,
            model: model_handle,
            controller: InputController {
                yaw: orientation.to_euler().y,
                ..Default::default()
            },
            lower_body_machine: locomotion_machine,
            health_cylinder,
            upper_body_machine: combat_machine,
            spine: scene.graph.find_by_name(model_handle, "mixamorig:Spine"),
            hips: scene.graph.find_by_name(model_handle, "mixamorig:Hips"),
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
            collider,
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
            in_air_time: 0.0,
            velocity: Default::default(),
            run_factor: 0.0,
            target_run_factor: 0.0,
            target_velocity: Default::default(),
            weapon_display,
            last_health: 100.0,
            health_color_gradient: make_color_gradient(),
            item_display,
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
            journal_display,
            journal: Journal::new(),
        }
    }

    pub fn persistent_data(&self, weapons: &WeaponContainer) -> PlayerPersistentData {
        PlayerPersistentData {
            inventory: self.inventory.clone(),
            health: self.health,
            current_weapon: self.current_weapon,
            weapons: self
                .weapons
                .iter()
                .map(|w| weapons[*w].get_kind())
                .collect::<Vec<_>>(),
        }
    }

    pub fn camera_controller(&self) -> &CameraController {
        &self.camera_controller
    }

    pub fn can_be_removed(&self, _scene: &Scene) -> bool {
        self.health <= 0.0
    }

    fn check_items(
        &mut self,
        self_handle: Handle<Actor>,
        scene: &mut Scene,
        items: &ItemContainer,
    ) {
        for (item_handle, item) in items.pair_iter() {
            let self_position = scene.graph[self.pivot].global_position();
            let item_position = scene.graph[item.get_pivot()].global_position();

            let distance = (item_position - self_position).norm();
            if distance < 0.75 {
                self.sender
                    .as_ref()
                    .unwrap()
                    .send(Message::ShowItemDisplay {
                        item: item.get_kind(),
                        count: item.stack_size,
                    })
                    .unwrap();

                if self.controller.action {
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::PickUpItem {
                            actor: self_handle,
                            item: item_handle,
                        })
                        .unwrap();

                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::SyncInventory)
                        .unwrap();

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

    fn handle_weapon_grab_signal(&mut self, self_handle: Handle<Actor>, scene: &mut Scene) {
        while let Some(event) = scene
            .animations
            .get_mut(self.upper_body_machine.grab_animation)
            .pop_event()
        {
            if event.signal_id == UpperBodyMachine::GRAB_WEAPON_SIGNAL {
                match self.weapon_change_direction {
                    RequiredWeapon::None => (),
                    RequiredWeapon::Next => self.next_weapon(),
                    RequiredWeapon::Previous => self.prev_weapon(),
                    RequiredWeapon::Specific(kind) => {
                        self.sender
                            .as_ref()
                            .unwrap()
                            .send(Message::GrabWeapon {
                                kind,
                                actor: self_handle,
                            })
                            .unwrap();
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

    fn handle_toss_grenade_signal(&mut self, self_handle: Handle<Actor>, scene: &mut Scene) {
        while let Some(event) = scene
            .animations
            .get_mut(self.upper_body_machine.toss_grenade_animation)
            .pop_event()
        {
            if event.signal_id == UpperBodyMachine::TOSS_GRENADE_SIGNAL {
                let position = scene.graph[self.weapon_pivot].global_position();
                let direction = scene.graph[self.camera_controller.camera()].look_vector();

                if self.inventory.try_extract_exact_items(ItemKind::Grenade, 1) == 1 {
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::CreateProjectile {
                            kind: ProjectileKind::Grenade,
                            position,
                            direction,
                            initial_velocity: direction.scale(15.0),
                            shooter: Shooter::Actor(self_handle),
                        })
                        .unwrap();
                }
            }
        }
    }

    fn update_velocity(&mut self, scene: &Scene, can_move: bool, dt: f32) {
        let pivot = &scene.graph[self.pivot];

        let look_vector = pivot
            .look_vector()
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_else(Vector3::z);

        let side_vector = pivot
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

    fn current_weapon_kind(&self, weapons: &WeaponContainer) -> CombatWeaponKind {
        if self.current_weapon().is_some() {
            match weapons[self.current_weapon()].get_kind() {
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
        mesh.surfaces_mut()
            .first_mut()
            .unwrap()
            .set_color(self.health_color_gradient.get_color(self.health / 100.0));
    }

    fn update_animation_machines(
        &mut self,
        dt: f32,
        scene: &mut Scene,
        is_walking: bool,
        is_jumping: bool,
        has_ground_contact: bool,
        weapons: &WeaponContainer,
    ) {
        let weapon_kind = self.current_weapon_kind(weapons);

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
            self.sender.clone().unwrap(),
            has_ground_contact,
            self.collider,
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

    fn update_shooting(&mut self, scene: &mut Scene, weapons: &WeaponContainer, time: GameTime) {
        if let Some(&current_weapon_handle) = self
            .character
            .weapons
            .get(self.character.current_weapon as usize)
        {
            if self.upper_body_machine.machine.active_state() == self.upper_body_machine.aim_state {
                let weapon = &weapons[current_weapon_handle];
                weapon.laser_sight().set_visible(true, &mut scene.graph);
                scene.graph[self.weapon_display]
                    .set_visibility(true)
                    .local_transform_mut()
                    .set_position(weapon.definition.ammo_indicator_offset());

                if self.controller.shoot && weapon.can_shoot(time) {
                    let ammo_per_shot = weapons[current_weapon_handle]
                        .definition
                        .ammo_consumption_per_shot;

                    if self
                        .inventory
                        .try_extract_exact_items(ItemKind::Ammo, ammo_per_shot)
                        == ammo_per_shot
                    {
                        self.character
                            .sender
                            .as_ref()
                            .unwrap()
                            .send(Message::ShootWeapon {
                                weapon: current_weapon_handle,
                                direction: None,
                            })
                            .unwrap();

                        self.camera_controller.request_shake_camera();
                        self.v_recoil
                            .set_target(weapon.definition.gen_v_recoil_angle());
                        self.h_recoil
                            .set_target(weapon.definition.gen_h_recoil_angle());
                    }
                }
            } else {
                weapons[current_weapon_handle]
                    .laser_sight()
                    .set_visible(false, &mut scene.graph);
                scene.graph[self.weapon_display].set_visibility(false);
            }
        }

        self.v_recoil.update(time.delta);
        self.h_recoil.update(time.delta);
    }

    fn can_move(&self) -> bool {
        self.lower_body_machine.machine.active_state() != self.lower_body_machine.fall_state
            && self.lower_body_machine.machine.active_state() != self.lower_body_machine.land_state
    }

    fn apply_weapon_angular_correction(
        &mut self,
        scene: &mut Scene,
        can_move: bool,
        dt: f32,
        weapons: &WeaponContainer,
    ) {
        if self.controller.aim {
            let (pitch_correction, yaw_correction) =
                if let Some(weapon) = weapons.try_get(self.current_weapon()) {
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

    pub fn update(&mut self, self_handle: Handle<Actor>, context: &mut UpdateContext) {
        let UpdateContext {
            time,
            scene,
            weapons,
            items,
            ..
        } = context;

        self.update_health_cylinder(scene);

        let has_ground_contact = self.has_ground_contact(&scene.physics);
        let is_walking = self.is_walking();
        let is_jumping = has_ground_contact && self.controller.jump;
        let position = **scene.graph[self.pivot].local_transform().position();

        self.update_animation_machines(
            time.delta,
            scene,
            is_walking,
            is_jumping,
            has_ground_contact,
            weapons,
        );

        let quat_yaw = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.controller.yaw);

        let is_running = self.is_running(scene);

        if !self.is_dead() {
            if is_running {
                self.target_run_factor = 1.0;
            } else {
                self.target_run_factor = 0.0;
            }
            self.run_factor += (self.target_run_factor - self.run_factor) * 0.1;

            let can_move = self.can_move();
            self.update_velocity(scene, can_move, time.delta);
            let new_y_vel = self.handle_jump_signal(scene, time.delta);
            self.handle_weapon_grab_signal(self_handle, scene);
            self.handle_put_back_weapon_end_signal(scene);
            self.handle_toss_grenade_signal(self_handle, scene);

            let body = scene.physics.bodies.get_mut(&self.body.unwrap()).unwrap();
            body.set_angvel(Default::default(), true);
            if let Some(new_y_vel) = new_y_vel {
                body.set_linvel(
                    Vector3::new(
                        self.velocity.x / time.delta,
                        new_y_vel / time.delta,
                        self.velocity.z / time.delta,
                    ),
                    true,
                );
            } else {
                body.set_linvel(
                    Vector3::new(
                        self.velocity.x / time.delta,
                        body.linvel().y,
                        self.velocity.z / time.delta,
                    ),
                    true,
                );
            }

            if self.controller.aim {
                self.spine_pitch.set_target(self.controller.pitch);
            } else {
                self.spine_pitch.set_target(0.0);
            }

            self.spine_pitch.update(time.delta);

            if can_move && (is_walking || self.controller.aim) {
                // Since we have free camera while not moving, we have to sync rotation of pivot
                // with rotation of camera so character will start moving in look direction.
                let mut current_position = *body.position();
                current_position.rotation = quat_yaw;
                body.set_position(current_position, true);

                // Apply additional rotation to model - it will turn in front of walking direction.
                let angle = self.calculate_model_angle();

                self.model_yaw
                    .set_target(angle.to_radians())
                    .update(time.delta);

                let mut additional_hips_rotation = Default::default();
                if self.controller.aim {
                    scene.graph[self.model]
                        .local_transform_mut()
                        .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0));

                    let spine_transform = scene.graph[self.spine].local_transform_mut();
                    spine_transform.set_rotation(
                        **spine_transform.rotation()
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
                    scene.graph[self.model].local_transform_mut().set_rotation(
                        UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.model_yaw.angle),
                    );

                    scene.graph[self.spine].local_transform_mut().set_rotation(
                        UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.spine_pitch.angle)
                            * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0),
                    );
                }

                scene.graph[self.hips].local_transform_mut().set_rotation(
                    additional_hips_rotation
                        * UnitQuaternion::from_axis_angle(
                            &Vector3::x_axis(),
                            math::lerpf(5.0f32.to_radians(), 17.0f32.to_radians(), self.run_factor),
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
                    scene.animations.get_mut(animation).set_speed(walk_dir);
                }
            }

            self.apply_weapon_angular_correction(scene, can_move, time.delta, weapons);

            if has_ground_contact {
                self.in_air_time = 0.0;
            } else {
                self.in_air_time += time.delta;
            }

            if !has_ground_contact {
                for &land_animation in &[
                    self.lower_body_machine.land_animation,
                    self.upper_body_machine.land_animation,
                ] {
                    scene.animations.get_mut(land_animation).rewind();
                }
            }

            scene.graph[self.item_display].set_visibility(false);

            self.check_items(self_handle, scene, items);
            self.update_shooting(scene, weapons, *time);

            let spine_transform = scene.graph[self.spine].local_transform_mut();
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
                scene.animations.get_mut(dying_animation).set_enabled(true);
            }

            // Lock player on the place he died.
            let body = scene.physics.bodies.get_mut(&self.body.unwrap()).unwrap();
            body.set_angvel(Default::default(), true);
            body.set_linvel(Vector3::new(0.0, body.linvel().y, 0.0), true);
        }

        self.camera_controller.update(
            position + self.velocity,
            self.controller.pitch,
            quat_yaw,
            is_walking,
            is_running,
            self.controller.aim,
            self.collider,
            scene,
            *time,
        );
    }

    pub fn process_input_event(
        &mut self,
        event: &Event<()>,
        dt: f32,
        scene: &mut Scene,
        weapons: &WeaponContainer,
        control_scheme: &ControlScheme,
    ) {
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
                    let mouse_sens = control_scheme.mouse_sens * dt;
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
            && scene.animations[self.upper_body_machine.grab_animation].has_ended()
            && self.weapons.len() > 1;

        let current_weapon_kind = if self.current_weapon().is_some() {
            Some(weapons[self.current_weapon()].get_kind())
        } else {
            None
        };

        let mut weapon_change_direction = None;

        if let Some((button, state)) = button_state {
            if button == control_scheme.aim.button {
                self.controller.aim = state == ElementState::Pressed;
                if state == ElementState::Pressed {
                    scene.graph[self.inventory_display].set_visibility(false);
                    scene.graph[self.journal_display].set_visibility(false);
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
                let jump_anim = scene.animations.get(self.lower_body_machine.jump_animation);
                let can_jump = !jump_anim.is_enabled() || jump_anim.has_ended();

                if state == ElementState::Pressed && can_jump {
                    // Rewind jump animation to beginning before jump.
                    scene
                        .animations
                        .get_mut(self.lower_body_machine.jump_animation)
                        .set_enabled(true)
                        .rewind();
                    scene
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
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::SwitchFlashLight {
                            weapon: current_weapon,
                        })
                        .unwrap();
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
                        scene
                            .animations
                            .get_mut(self.upper_body_machine.toss_grenade_animation)
                            .set_enabled(true)
                            .rewind();
                    }
                }
            } else if button == control_scheme.shoot.button {
                self.controller.shoot = state == ElementState::Pressed;
            } else if button == control_scheme.action.button {
                self.controller.action = state == ElementState::Pressed;
            } else if button == control_scheme.inventory.button
                && state == ElementState::Pressed
                && !self.controller.aim
            {
                scene.graph[self.journal_display].set_visibility(false);

                let inventory = &mut scene.graph[self.inventory_display];
                let new_visibility = !inventory.visibility();
                inventory.set_visibility(new_visibility);
                if new_visibility {
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::SyncInventory)
                        .unwrap();
                }
            } else if button == control_scheme.journal.button
                && state == ElementState::Pressed
                && !self.controller.aim
            {
                scene.graph[self.inventory_display].set_visibility(false);

                let journal = &mut scene.graph[self.journal_display];
                let new_visibility = !journal.visibility();
                journal.set_visibility(new_visibility);
                if new_visibility {
                    self.sender
                        .as_ref()
                        .unwrap()
                        .send(Message::SyncJournal)
                        .unwrap();
                }
            }
        }

        if let Some(weapon_change_direction) = weapon_change_direction {
            self.weapon_change_direction = weapon_change_direction;

            scene
                .animations
                .get_mut(self.upper_body_machine.put_back_animation)
                .rewind();

            scene
                .animations
                .get_mut(self.upper_body_machine.grab_animation)
                .set_enabled(false)
                .rewind();
        }
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
        scene.graph[self.weapon_display]
            .as_mesh_mut()
            .surfaces_mut()
            .first_mut()
            .unwrap()
            .set_diffuse_texture(Some(display_texture));

        scene.graph[self.inventory_display]
            .as_mesh_mut()
            .surfaces_mut()
            .first_mut()
            .unwrap()
            .set_diffuse_texture(Some(inventory_texture));

        scene.graph[self.journal_display]
            .as_mesh_mut()
            .surfaces_mut()
            .first_mut()
            .unwrap()
            .set_diffuse_texture(Some(journal_texture));

        scene.graph[self.item_display]
            .as_sprite_mut()
            .set_texture(Some(item_texture));
    }
}
