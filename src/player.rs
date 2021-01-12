use crate::{
    character::Character,
    control_scheme::{ControlButton, ControlScheme},
    level::UpdateContext,
    message::Message,
};
use rg3d::core::visitor::{Visit, VisitResult, Visitor};
use rg3d::{
    animation::{
        machine::{BlendPose, Machine, Parameter, PoseNode, PoseWeight, State, Transition},
        Animation, AnimationSignal,
    },
    core::{
        algebra::{Isometry3, UnitQuaternion, Vector3},
        math::{ray::Ray, SmoothAngle},
        pool::Handle,
    },
    engine::resource_manager::ResourceManager,
    event::{DeviceEvent, ElementState, Event, MouseScrollDelta, WindowEvent},
    physics::{dynamics::RigidBodyBuilder, geometry::ColliderBuilder},
    resource::{model::Model, texture::TextureWrapMode},
    scene::{
        base::BaseBuilder,
        camera::{CameraBuilder, SkyBox},
        graph::Graph,
        node::Node,
        physics::RayCastOptions,
        transform::TransformBuilder,
        ColliderHandle, Scene,
    },
};
use std::{
    ops::{Deref, DerefMut},
    sync::{mpsc::Sender, Arc, RwLock},
};

pub fn create_play_animation_state(
    animation_resource: Model,
    name: &str,
    machine: &mut Machine,
    scene: &mut Scene,
    model: Handle<Node>,
) -> (Handle<Animation>, Handle<State>) {
    let animation = *animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    let node = machine.add_node(PoseNode::make_play_animation(animation));
    let state = machine.add_state(State::new(name, node));
    (animation, state)
}

#[derive(Default)]
pub struct UpperBodyMachine {
    pub machine: Machine,
    pub aim_state: Handle<State>,
    pub toss_grenade_state: Handle<State>,
    pub jump_animation: Handle<Animation>,
    pub walk_animation: Handle<Animation>,
    pub land_animation: Handle<Animation>,
    pub toss_grenade_animation: Handle<Animation>,
    pub put_back_animation: Handle<Animation>,
    pub grab_animation: Handle<Animation>,
}

impl Visit for UpperBodyMachine {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.machine.visit("Machine", visitor)?;
        self.aim_state.visit("AimState", visitor)?;
        self.toss_grenade_animation
            .visit("TossGrenadeAnimation", visitor)?;
        self.jump_animation.visit("JumpAnimation", visitor)?;
        self.walk_animation.visit("WalkAnimation", visitor)?;
        self.land_animation.visit("LandAnimation", visitor)?;
        self.toss_grenade_state.visit("TossGrenadeState", visitor)?;
        self.put_back_animation.visit("PutBackAnimation", visitor)?;
        self.grab_animation.visit("GrabAnimation", visitor)?;

        visitor.leave_region()
    }
}

fn disable_leg_tracks(
    animation: Handle<Animation>,
    root: Handle<Node>,
    leg_name: &str,
    scene: &mut Scene,
) {
    let animation = scene.animations.get_mut(animation);
    animation.set_tracks_enabled_from(
        scene.graph.find_by_name(root, leg_name),
        false,
        &scene.graph,
    )
}

/// Creates a camera at given position with a skybox.
pub async fn create_camera(
    resource_manager: ResourceManager,
    position: Vector3<f32>,
    graph: &mut Graph,
) -> Handle<Node> {
    // Load skybox textures in parallel.
    let (front, back, left, right, top, bottom) = rg3d::futures::join!(
        resource_manager
            .request_texture("data/textures/skyboxes/DarkStormy/DarkStormyFront2048.png"),
        resource_manager
            .request_texture("data/textures/skyboxes/DarkStormy/DarkStormyBack2048.png"),
        resource_manager
            .request_texture("data/textures/skyboxes/DarkStormy/DarkStormyLeft2048.png"),
        resource_manager
            .request_texture("data/textures/skyboxes/DarkStormy/DarkStormyRight2048.png"),
        resource_manager.request_texture("data/textures/skyboxes/DarkStormy/DarkStormyUp2048.png"),
        resource_manager
            .request_texture("data/textures/skyboxes/DarkStormy/DarkStormyDown2048.png")
    );

    // Unwrap everything.
    let skybox = SkyBox {
        front: Some(front.unwrap()),
        back: Some(back.unwrap()),
        left: Some(left.unwrap()),
        right: Some(right.unwrap()),
        top: Some(top.unwrap()),
        bottom: Some(bottom.unwrap()),
    };

    // Set S and T coordinate wrap mode, ClampToEdge will remove any possible seams on edges
    // of the skybox.
    for skybox_texture in skybox.textures().iter().filter_map(|t| t.clone()) {
        let mut data = skybox_texture.data_ref();
        data.set_s_wrap_mode(TextureWrapMode::ClampToEdge);
        data.set_t_wrap_mode(TextureWrapMode::ClampToEdge);
    }

    // Camera is our eyes in the world - you won't see anything without it.
    CameraBuilder::new(
        BaseBuilder::new().with_local_transform(
            TransformBuilder::new()
                .with_local_position(position)
                .build(),
        ),
    )
    .with_skybox(skybox)
    .build(graph)
}

#[derive(Eq, PartialEq, Copy, Clone)]
pub enum CombatMachineWeapon {
    Pistol,
    Rifle,
}

pub struct UpperBodyMachineInput {
    is_walking: bool,
    is_jumping: bool,
    has_ground_contact: bool,
    is_aiming: bool,
    toss_grenade: bool,
    weapon: CombatMachineWeapon,
    put_back_weapon: bool,
    grab_new_weapon: bool,
}

impl UpperBodyMachine {
    const WALK_TO_AIM: &'static str = "WalkToAim";
    const IDLE_TO_AIM: &'static str = "IdleToAim";
    const AIM_TO_IDLE: &'static str = "AimToIdle";
    const AIM_TO_WALK: &'static str = "AimToWalk";

    const WALK_TO_IDLE: &'static str = "WalkToIdle";
    const WALK_TO_JUMP: &'static str = "WalkToJump";
    const IDLE_TO_WALK: &'static str = "IdleToWalk";
    const IDLE_TO_JUMP: &'static str = "IdleToJump";
    const JUMP_TO_FALL: &'static str = "JumpToFall";
    const WALK_TO_FALL: &'static str = "WalkToFall";
    const IDLE_TO_FALL: &'static str = "IdleToFall";
    const FALL_TO_LAND: &'static str = "FallToLand";
    const LAND_TO_IDLE: &'static str = "LandToIdle";
    const TOSS_GRENADE_TO_AIM: &'static str = "TossGrenadeToAim";
    const AIM_TO_TOSS_GRENADE: &'static str = "AimToTossGrenade";

    const AIM_TO_PUT_BACK: &'static str = "AimToPutBack";
    const WALK_TO_PUT_BACK: &'static str = "WalkToPutBack";
    const IDLE_TO_PUT_BACK: &'static str = "IdleToPutBack";

    const PUT_BACK_TO_IDLE: &'static str = "PutBackToIdle";
    const PUT_BACK_TO_WALK: &'static str = "PutBackToWalk";

    const PUT_BACK_TO_GRAB: &'static str = "PutBackToGrab";
    const GRAB_TO_IDLE: &'static str = "GrabToIdle";
    const GRAB_TO_WALK: &'static str = "GrabToWalk";

    const RIFLE_AIM_FACTOR: &'static str = "RifleAimFactor";
    const PISTOL_AIM_FACTOR: &'static str = "PistolAimFactor";

    pub const GRAB_WEAPON_SIGNAL: u64 = 1;

    pub async fn new(
        scene: &mut Scene,
        model: Handle<Node>,
        resource_manager: ResourceManager,
    ) -> Self {
        let mut machine = Machine::new();

        let (
            walk_animation_resource,
            idle_animation_resource,
            jump_animation_resource,
            falling_animation_resource,
            landing_animation_resource,
            aim_rifle_animation_resource,
            aim_pistol_animation_resource,
            toss_grenade_animation_resource,
            put_back_animation_resource,
            grab_animation_resource,
        ) = rg3d::futures::join!(
            resource_manager.request_model("data/animations/agent_walk_rifle.fbx"),
            resource_manager.request_model("data/animations/agent_idle.fbx"),
            resource_manager.request_model("data/animations/agent_jump.fbx"),
            resource_manager.request_model("data/animations/agent_falling.fbx"),
            resource_manager.request_model("data/animations/agent_landing.fbx"),
            resource_manager.request_model("data/animations/agent_aim_rifle.fbx"),
            resource_manager.request_model("data/animations/agent_aim_pistol.fbx"),
            resource_manager.request_model("data/animations/agent_toss_grenade.fbx"),
            resource_manager.request_model("data/animations/agent_put_back.fbx"),
            resource_manager.request_model("data/animations/agent_grab.fbx"),
        );

        let aim_rifle_animation = *aim_rifle_animation_resource
            .unwrap()
            .retarget_animations(model, scene)
            .get(0)
            .unwrap();
        let aim_rifle_animation_node =
            machine.add_node(PoseNode::make_play_animation(aim_rifle_animation));

        let aim_pistol_animation = *aim_pistol_animation_resource
            .unwrap()
            .retarget_animations(model, scene)
            .get(0)
            .unwrap();
        let aim_pistol_animation_node =
            machine.add_node(PoseNode::make_play_animation(aim_pistol_animation));

        let aim_node = machine.add_node(PoseNode::make_blend_animations(vec![
            BlendPose::new(
                PoseWeight::Parameter(Self::RIFLE_AIM_FACTOR.to_owned()),
                aim_rifle_animation_node,
            ),
            BlendPose::new(
                PoseWeight::Parameter(Self::PISTOL_AIM_FACTOR.to_owned()),
                aim_pistol_animation_node,
            ),
        ]));
        let aim_state = machine.add_state(State::new("Aim", aim_node));

        let (toss_grenade_animation, toss_grenade_state) = create_play_animation_state(
            toss_grenade_animation_resource.unwrap(),
            "TossGrenade",
            &mut machine,
            scene,
            model,
        );

        let (walk_animation, walk_state) = create_play_animation_state(
            walk_animation_resource.unwrap(),
            "Walk",
            &mut machine,
            scene,
            model,
        );

        let (idle_animation, idle_state) = create_play_animation_state(
            idle_animation_resource.unwrap(),
            "Idle",
            &mut machine,
            scene,
            model,
        );

        let (jump_animation, jump_state) = create_play_animation_state(
            jump_animation_resource.unwrap(),
            "Jump",
            &mut machine,
            scene,
            model,
        );

        let (fall_animation, fall_state) = create_play_animation_state(
            falling_animation_resource.unwrap(),
            "Fall",
            &mut machine,
            scene,
            model,
        );

        let (land_animation, land_state) = create_play_animation_state(
            landing_animation_resource.unwrap(),
            "Land",
            &mut machine,
            scene,
            model,
        );

        let (put_back_animation, put_back_state) = create_play_animation_state(
            put_back_animation_resource.unwrap(),
            "PutBack",
            &mut machine,
            scene,
            model,
        );

        let (grab_animation, grab_state) = create_play_animation_state(
            grab_animation_resource.unwrap(),
            "Grab",
            &mut machine,
            scene,
            model,
        );

        // Some animations must not be looped.
        scene.animations.get_mut(jump_animation).set_loop(false);
        scene.animations.get_mut(land_animation).set_loop(false);
        scene
            .animations
            .get_mut(grab_animation)
            .set_loop(false)
            .add_signal(AnimationSignal::new(Self::GRAB_WEAPON_SIGNAL, 0.1));
        scene.animations.get_mut(put_back_animation).set_loop(false);
        scene
            .animations
            .get_mut(toss_grenade_animation)
            .set_loop(false);

        machine.add_transition(Transition::new(
            "Walk->Idle",
            walk_state,
            idle_state,
            0.30,
            Self::WALK_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "Walk->Jump",
            walk_state,
            jump_state,
            0.20,
            Self::WALK_TO_JUMP,
        ));
        machine.add_transition(Transition::new(
            "Idle->Walk",
            idle_state,
            walk_state,
            0.30,
            Self::IDLE_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "Idle->Jump",
            idle_state,
            jump_state,
            0.25,
            Self::IDLE_TO_JUMP,
        ));
        machine.add_transition(Transition::new(
            "Falling->Landing",
            fall_state,
            land_state,
            0.20,
            Self::FALL_TO_LAND,
        ));
        machine.add_transition(Transition::new(
            "Landing->Idle",
            land_state,
            idle_state,
            0.20,
            Self::LAND_TO_IDLE,
        ));

        // Falling state can be entered from: Jump, Walk, Idle states.
        machine.add_transition(Transition::new(
            "Jump->Falling",
            jump_state,
            fall_state,
            0.30,
            Self::JUMP_TO_FALL,
        ));
        machine.add_transition(Transition::new(
            "Walk->Falling",
            walk_state,
            fall_state,
            0.30,
            Self::WALK_TO_FALL,
        ));
        machine.add_transition(Transition::new(
            "Idle->Falling",
            idle_state,
            fall_state,
            0.20,
            Self::IDLE_TO_FALL,
        ));
        machine.add_transition(Transition::new(
            "Idle->Aim",
            idle_state,
            aim_state,
            0.20,
            Self::IDLE_TO_AIM,
        ));
        machine.add_transition(Transition::new(
            "Walk->Aim",
            walk_state,
            aim_state,
            0.20,
            Self::WALK_TO_AIM,
        ));
        machine.add_transition(Transition::new(
            "Aim->Idle",
            aim_state,
            idle_state,
            0.20,
            Self::AIM_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "Walk->Aim",
            aim_state,
            walk_state,
            0.20,
            Self::AIM_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "Aim->TossGrenade",
            aim_state,
            toss_grenade_state,
            0.20,
            Self::AIM_TO_TOSS_GRENADE,
        ));
        machine.add_transition(Transition::new(
            "TossGrenade->Aim",
            toss_grenade_state,
            aim_state,
            0.20,
            Self::TOSS_GRENADE_TO_AIM,
        ));

        machine.add_transition(Transition::new(
            "Aim->PutBack",
            aim_state,
            put_back_state,
            0.20,
            Self::AIM_TO_PUT_BACK,
        ));
        machine.add_transition(Transition::new(
            "Walk->PutBack",
            walk_state,
            put_back_state,
            0.20,
            Self::WALK_TO_PUT_BACK,
        ));
        machine.add_transition(Transition::new(
            "Idle->PutBack",
            idle_state,
            put_back_state,
            0.20,
            Self::IDLE_TO_PUT_BACK,
        ));

        machine.add_transition(Transition::new(
            "PutBack->Idle",
            put_back_state,
            idle_state,
            0.20,
            Self::PUT_BACK_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "PutBack->Walk",
            put_back_state,
            walk_state,
            0.20,
            Self::PUT_BACK_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "PutBack->Grab",
            put_back_state,
            grab_state,
            0.20,
            Self::PUT_BACK_TO_GRAB,
        ));
        machine.add_transition(Transition::new(
            "Grab->Idle",
            grab_state,
            idle_state,
            0.20,
            Self::GRAB_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "Grab->Walk",
            grab_state,
            walk_state,
            0.20,
            Self::GRAB_TO_WALK,
        ));

        for leg in &["mixamorig:LeftUpLeg", "mixamorig:RightUpLeg"] {
            for &animation in &[
                aim_pistol_animation,
                aim_rifle_animation,
                toss_grenade_animation,
                walk_animation,
                idle_animation,
                jump_animation,
                fall_animation,
                land_animation,
                grab_animation,
                put_back_animation,
            ] {
                disable_leg_tracks(animation, model, leg, scene);
            }
        }

        machine.set_entry_state(idle_state);

        Self {
            machine,
            aim_state,
            toss_grenade_state,
            jump_animation,
            walk_animation,
            land_animation,
            toss_grenade_animation,
            put_back_animation,
            grab_animation,
        }
    }

    pub fn apply(&mut self, scene: &mut Scene, dt: f32, input: UpperBodyMachineInput) {
        self.machine
            // Update parameters which will be used by transitions.
            .set_parameter(Self::IDLE_TO_WALK, Parameter::Rule(input.is_walking))
            .set_parameter(Self::WALK_TO_IDLE, Parameter::Rule(!input.is_walking))
            .set_parameter(Self::WALK_TO_JUMP, Parameter::Rule(input.is_jumping))
            .set_parameter(Self::IDLE_TO_JUMP, Parameter::Rule(input.is_jumping))
            .set_parameter(
                Self::JUMP_TO_FALL,
                Parameter::Rule(scene.animations.get(self.jump_animation).has_ended()),
            )
            .set_parameter(
                Self::WALK_TO_FALL,
                Parameter::Rule(!input.has_ground_contact),
            )
            .set_parameter(
                Self::IDLE_TO_FALL,
                Parameter::Rule(!input.has_ground_contact),
            )
            .set_parameter(
                Self::FALL_TO_LAND,
                Parameter::Rule(input.has_ground_contact),
            )
            .set_parameter(
                Self::LAND_TO_IDLE,
                Parameter::Rule(scene.animations.get(self.land_animation).has_ended()),
            )
            .set_parameter(
                Self::IDLE_TO_AIM,
                Parameter::Rule(input.is_aiming && input.has_ground_contact),
            )
            .set_parameter(
                Self::WALK_TO_AIM,
                Parameter::Rule(input.is_aiming && input.has_ground_contact),
            )
            .set_parameter(
                Self::AIM_TO_IDLE,
                Parameter::Rule(!input.is_aiming || !input.has_ground_contact),
            )
            .set_parameter(
                Self::AIM_TO_WALK,
                Parameter::Rule(!input.is_aiming || !input.has_ground_contact),
            )
            .set_parameter(
                Self::AIM_TO_PUT_BACK,
                Parameter::Rule(input.is_aiming && input.put_back_weapon),
            )
            .set_parameter(
                Self::WALK_TO_PUT_BACK,
                Parameter::Rule(input.put_back_weapon),
            )
            .set_parameter(
                Self::IDLE_TO_PUT_BACK,
                Parameter::Rule(input.put_back_weapon),
            )
            .set_parameter(
                Self::PUT_BACK_TO_IDLE,
                Parameter::Rule(!input.put_back_weapon),
            )
            .set_parameter(
                Self::PUT_BACK_TO_WALK,
                Parameter::Rule(!input.put_back_weapon && input.is_walking),
            )
            .set_parameter(
                Self::PUT_BACK_TO_GRAB,
                Parameter::Rule(
                    input.grab_new_weapon
                        && scene.animations.get(self.put_back_animation).has_ended(),
                ),
            )
            .set_parameter(Self::GRAB_TO_IDLE, Parameter::Rule(!input.grab_new_weapon))
            .set_parameter(
                Self::GRAB_TO_WALK,
                Parameter::Rule(!input.grab_new_weapon && input.is_walking),
            )
            .set_parameter(
                Self::PISTOL_AIM_FACTOR,
                Parameter::Weight(if input.weapon == CombatMachineWeapon::Pistol {
                    1.0
                } else {
                    0.0
                }),
            )
            .set_parameter(
                Self::RIFLE_AIM_FACTOR,
                Parameter::Weight(if input.weapon == CombatMachineWeapon::Rifle {
                    1.0
                } else {
                    0.0
                }),
            )
            .set_parameter(
                Self::TOSS_GRENADE_TO_AIM,
                Parameter::Rule(
                    !input.toss_grenade
                        && scene
                            .animations
                            .get(self.toss_grenade_animation)
                            .has_ended(),
                ),
            )
            .set_parameter(
                Self::AIM_TO_TOSS_GRENADE,
                Parameter::Rule(input.toss_grenade && input.is_aiming),
            )
            .evaluate_pose(&scene.animations, dt)
            .apply(&mut scene.graph);
    }
}

#[derive(Default)]
pub struct LowerBodyMachine {
    pub machine: Machine,
    pub jump_animation: Handle<Animation>,
    pub walk_animation: Handle<Animation>,
    pub land_animation: Handle<Animation>,
    pub walk_state: Handle<State>,
    pub jump_state: Handle<State>,
    pub fall_state: Handle<State>,
    pub land_state: Handle<State>,
    pub walk_to_jump: Handle<Transition>,
    pub idle_to_jump: Handle<Transition>,
}

impl Visit for LowerBodyMachine {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.machine.visit("Machine", visitor)?;
        self.jump_animation.visit("JumpAnimation", visitor)?;
        self.walk_animation.visit("WalkAnimation", visitor)?;
        self.land_animation.visit("LandAnimation", visitor)?;
        self.walk_state.visit("WalkState", visitor)?;
        self.jump_state.visit("JumpState", visitor)?;
        self.fall_state.visit("FallState", visitor)?;
        self.land_state.visit("LandState", visitor)?;
        self.walk_to_jump.visit("WalkToJump", visitor)?;
        self.idle_to_jump.visit("IdleToJump", visitor)?;

        visitor.leave_region()
    }
}

pub struct LowerBodyMachineInput {
    is_walking: bool,
    is_jumping: bool,
    has_ground_contact: bool,
}

impl LowerBodyMachine {
    const WALK_TO_IDLE: &'static str = "WalkToIdle";
    const WALK_TO_JUMP: &'static str = "WalkToJump";
    const IDLE_TO_WALK: &'static str = "IdleToWalk";
    const IDLE_TO_JUMP: &'static str = "IdleToJump";
    const JUMP_TO_FALL: &'static str = "JumpToFall";
    const WALK_TO_FALL: &'static str = "WalkToFall";
    const IDLE_TO_FALL: &'static str = "IdleToFall";
    const FALL_TO_LAND: &'static str = "FallToLand";
    const LAND_TO_IDLE: &'static str = "LandToIdle";

    pub const JUMP_SIGNAL: u64 = 1;
    pub const LANDING_SIGNAL: u64 = 2;

    pub async fn new(
        scene: &mut Scene,
        model: Handle<Node>,
        resource_manager: ResourceManager,
    ) -> Self {
        let mut machine = Machine::new();

        // Load animations in parallel.
        let (
            walk_animation_resource,
            idle_animation_resource,
            jump_animation_resource,
            falling_animation_resource,
            landing_animation_resource,
        ) = rg3d::futures::join!(
            resource_manager.request_model("data/animations/agent_walk_rifle.fbx"),
            resource_manager.request_model("data/animations/agent_idle.fbx"),
            resource_manager.request_model("data/animations/agent_jump.fbx"),
            resource_manager.request_model("data/animations/agent_falling.fbx"),
            resource_manager.request_model("data/animations/agent_landing.fbx"),
        );

        let (walk_animation, walk_state) = create_play_animation_state(
            walk_animation_resource.unwrap(),
            "Walk",
            &mut machine,
            scene,
            model,
        );

        let (idle_animation, idle_state) = create_play_animation_state(
            idle_animation_resource.unwrap(),
            "Idle",
            &mut machine,
            scene,
            model,
        );

        let (jump_animation, jump_state) = create_play_animation_state(
            jump_animation_resource.unwrap(),
            "Jump",
            &mut machine,
            scene,
            model,
        );

        let (fall_animation, fall_state) = create_play_animation_state(
            falling_animation_resource.unwrap(),
            "Fall",
            &mut machine,
            scene,
            model,
        );

        let (land_animation, land_state) = create_play_animation_state(
            landing_animation_resource.unwrap(),
            "Land",
            &mut machine,
            scene,
            model,
        );

        scene
            .animations
            .get_mut(jump_animation)
            // Actual jump (applying force to physical body) must be synced with animation
            // so we have to be notified about this. This is where signals come into play
            // you can assign any signal in animation timeline and then in update loop you
            // can iterate over them and react appropriately.
            .add_signal(AnimationSignal::new(Self::JUMP_SIGNAL, 0.15))
            .set_loop(false);

        scene
            .animations
            .get_mut(land_animation)
            .add_signal(AnimationSignal::new(Self::LANDING_SIGNAL, 0.1))
            .set_loop(false);

        // Add transitions between states. This is the "heart" of animation blending state machine
        // it defines how it will respond to input parameters.
        machine.add_transition(Transition::new(
            "Walk->Idle",
            walk_state,
            idle_state,
            0.30,
            Self::WALK_TO_IDLE,
        ));
        let walk_to_jump = machine.add_transition(Transition::new(
            "Walk->Jump",
            walk_state,
            jump_state,
            0.20,
            Self::WALK_TO_JUMP,
        ));
        machine.add_transition(Transition::new(
            "Idle->Walk",
            idle_state,
            walk_state,
            0.30,
            Self::IDLE_TO_WALK,
        ));
        let idle_to_jump = machine.add_transition(Transition::new(
            "Idle->Jump",
            idle_state,
            jump_state,
            0.25,
            Self::IDLE_TO_JUMP,
        ));
        machine.add_transition(Transition::new(
            "Falling->Landing",
            fall_state,
            land_state,
            0.20,
            Self::FALL_TO_LAND,
        ));
        machine.add_transition(Transition::new(
            "Landing->Idle",
            land_state,
            idle_state,
            0.20,
            Self::LAND_TO_IDLE,
        ));

        // Falling state can be entered from: Jump, Walk, Idle states.
        machine.add_transition(Transition::new(
            "Jump->Falling",
            jump_state,
            fall_state,
            0.20,
            Self::JUMP_TO_FALL,
        ));
        machine.add_transition(Transition::new(
            "Walk->Falling",
            walk_state,
            fall_state,
            0.20,
            Self::WALK_TO_FALL,
        ));
        machine.add_transition(Transition::new(
            "Idle->Falling",
            idle_state,
            fall_state,
            0.20,
            Self::IDLE_TO_FALL,
        ));

        for &animation in &[
            walk_animation,
            idle_animation,
            jump_animation,
            fall_animation,
            land_animation,
        ] {
            scene.animations.get_mut(animation).set_tracks_enabled_from(
                scene.graph.find_by_name(model, "mixamorig:Spine"),
                false,
                &scene.graph,
            );
        }

        Self {
            machine,
            jump_animation,
            walk_animation,
            walk_state,
            jump_state,
            walk_to_jump,
            idle_to_jump,
            land_animation,
            fall_state,
            land_state,
        }
    }

    pub fn apply(&mut self, scene: &mut Scene, dt: f32, input: LowerBodyMachineInput) {
        self.machine
            // Update parameters which will be used by transitions.
            .set_parameter(Self::IDLE_TO_WALK, Parameter::Rule(input.is_walking))
            .set_parameter(Self::WALK_TO_IDLE, Parameter::Rule(!input.is_walking))
            .set_parameter(Self::WALK_TO_JUMP, Parameter::Rule(input.is_jumping))
            .set_parameter(Self::IDLE_TO_JUMP, Parameter::Rule(input.is_jumping))
            .set_parameter(
                Self::JUMP_TO_FALL,
                Parameter::Rule(scene.animations.get(self.jump_animation).has_ended()),
            )
            .set_parameter(
                Self::WALK_TO_FALL,
                Parameter::Rule(!input.has_ground_contact),
            )
            .set_parameter(
                Self::IDLE_TO_FALL,
                Parameter::Rule(!input.has_ground_contact),
            )
            .set_parameter(
                Self::FALL_TO_LAND,
                Parameter::Rule(input.has_ground_contact),
            )
            .set_parameter(
                Self::LAND_TO_IDLE,
                Parameter::Rule(scene.animations.get(self.land_animation).has_ended()),
            )
            .evaluate_pose(&scene.animations, dt)
            .apply(&mut scene.graph);
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

#[derive(Copy, Clone, PartialOrd, PartialEq, Eq, Ord)]
#[repr(u32)]
enum Direction {
    None,
    Next,
    Previous,
}

impl Direction {
    fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::None),
            1 => Ok(Self::Next),
            2 => Ok(Self::Previous),
            _ => Err(format!("Invalid Direction id {}!", id)),
        }
    }
}

impl Default for Direction {
    fn default() -> Self {
        Self::None
    }
}

#[derive(Default)]
pub struct Player {
    character: Character,
    camera_pivot: Handle<Node>,
    camera_hinge: Handle<Node>,
    camera: Handle<Node>,
    model: Handle<Node>,
    controller: InputController,
    lower_body_machine: LowerBodyMachine,
    upper_body_machine: UpperBodyMachine,
    model_yaw: SmoothAngle,
    spine_pitch: SmoothAngle,
    spine: Handle<Node>,
    hips: Handle<Node>,
    move_speed: f32,
    camera_offset: f32,
    collider: ColliderHandle,
    control_scheme: Option<Arc<RwLock<ControlScheme>>>,
    weapon_change_direction: Direction,
}

impl Visit for Player {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.character.visit("Character", visitor)?;
        self.camera_pivot.visit("CameraPivot", visitor)?;
        self.camera_hinge.visit("CameraHinge", visitor)?;
        self.camera.visit("Camera", visitor)?;
        self.model.visit("Model", visitor)?;
        self.lower_body_machine.visit("LowerBodyMachine", visitor)?;
        self.upper_body_machine.visit("UpperBodyMachine", visitor)?;
        self.model_yaw.visit("ModelYaw", visitor)?;
        self.spine_pitch.visit("SpinePitch", visitor)?;
        self.hips.visit("Hips", visitor)?;
        self.spine.visit("Spine", visitor)?;
        self.move_speed.visit("MoveSpeed", visitor)?;
        self.camera_offset.visit("CameraOffset", visitor)?;
        self.collider.visit("Collider", visitor)?;

        let mut direction = self.weapon_change_direction as u32;
        direction.visit("WeaponChangeDirection", visitor)?;
        if visitor.is_reading() {
            self.weapon_change_direction = Direction::from_id(direction)?;
        }

        visitor.leave_region()
    }
}

impl Player {
    pub async fn new(
        scene: &mut Scene,
        resource_manager: ResourceManager,
        position: Vector3<f32>,
        sender: Sender<Message>,
        control_scheme: Arc<RwLock<ControlScheme>>,
    ) -> Self {
        let body_radius = 0.2;
        let body_height = 0.25;
        let camera_offset = -0.8;

        let camera;
        let camera_hinge;
        let camera_pivot = BaseBuilder::new()
            .with_children(&[{
                camera_hinge = BaseBuilder::new()
                    .with_local_transform(
                        TransformBuilder::new()
                            .with_local_position(Vector3::new(-0.22, 0.25, 0.0))
                            .build(),
                    )
                    .with_children(&[{
                        camera = create_camera(
                            resource_manager.clone(),
                            Vector3::new(0.0, 0.0, camera_offset),
                            &mut scene.graph,
                        )
                        .await;
                        camera
                    }])
                    .build(&mut scene.graph);
                camera_hinge
            }])
            .build(&mut scene.graph);

        let model_resource = resource_manager
            .request_model("data/models/agent.fbx")
            .await
            .unwrap();

        let model_handle = model_resource.instantiate_geometry(scene);

        scene.graph[model_handle]
            .local_transform_mut()
            .set_position(Vector3::new(0.0, -body_height - body_radius, 0.0))
            // Our model is too big, fix it by scale.
            .set_scale(Vector3::new(0.005, 0.005, 0.005));

        let pivot = BaseBuilder::new()
            .with_children(&[model_handle])
            .build(&mut scene.graph);

        let capsule = ColliderBuilder::capsule_y(body_height, body_radius)
            .friction(0.0)
            .build();
        let body = scene.physics.add_body(
            RigidBodyBuilder::new_dynamic()
                .position(Isometry3::new(position, Default::default()))
                .build(),
        );
        let collider = scene.physics.add_collider(capsule, body);

        scene.physics_binder.bind(pivot, body.into());

        let locomotion_machine =
            LowerBodyMachine::new(scene, model_handle, resource_manager.clone()).await;

        let combat_machine =
            UpperBodyMachine::new(scene, model_handle, resource_manager.clone()).await;

        scene.graph.update_hierarchical_data();

        let hand = scene
            .graph
            .find_by_name(model_handle, "mixamorig:RightHand");

        let hand_scale = scene.graph.global_scale(hand);

        let weapon_pivot = BaseBuilder::new()
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
            .build(&mut scene.graph);

        scene.graph.link_nodes(weapon_pivot, hand);

        Self {
            character: Character {
                pivot,
                body: body.into(),
                weapon_pivot,
                sender: Some(sender),
                ..Default::default()
            },
            model: model_handle,
            camera_pivot,
            controller: Default::default(),
            lower_body_machine: locomotion_machine,
            camera_hinge,
            camera,
            upper_body_machine: combat_machine,
            spine: scene.graph.find_by_name(model_handle, "mixamorig:Spine"),
            hips: scene.graph.find_by_name(model_handle, "mixamorig:Hips"),
            model_yaw: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 10.0,
            },
            move_speed: 1.0,
            spine_pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 10.0,
            },
            camera_offset,
            collider,
            control_scheme: Some(control_scheme),
            weapon_change_direction: Direction::None,
        }
    }

    pub fn set_control_scheme(&mut self, control_scheme: Arc<RwLock<ControlScheme>>) {
        self.control_scheme = Some(control_scheme);
    }

    pub fn camera(&self) -> Handle<Node> {
        self.camera
    }

    pub fn can_be_removed(&self) -> bool {
        self.health <= 0.0
    }

    pub fn update(&mut self, context: &mut UpdateContext) {
        let UpdateContext { time, scene, .. } = context;

        let body = scene.physics.bodies.get_mut(self.body.into()).unwrap();

        let mut has_ground_contact = false;
        if let Some(iterator) = scene
            .physics
            .narrow_phase
            .contacts_with(body.colliders()[0])
        {
            'outer_loop: for (_, _, contact) in iterator {
                for manifold in contact.manifolds.iter() {
                    if manifold.local_n1.y > 0.7 {
                        has_ground_contact = true;
                        break 'outer_loop;
                    }
                }
            }
        }

        let is_walking = self.controller.walk_backward
            || self.controller.walk_forward
            || self.controller.walk_right
            || self.controller.walk_left;
        let is_jumping = has_ground_contact && self.controller.jump;

        self.lower_body_machine.apply(
            scene,
            time.delta,
            LowerBodyMachineInput {
                is_walking,
                is_jumping,
                has_ground_contact,
            },
        );

        self.upper_body_machine.apply(
            scene,
            time.delta,
            UpperBodyMachineInput {
                is_walking,
                is_jumping,
                has_ground_contact,
                is_aiming: self.controller.aim,
                toss_grenade: self.controller.toss_grenade,
                weapon: CombatMachineWeapon::Rifle,
                put_back_weapon: self.weapon_change_direction != Direction::None,
                grab_new_weapon: self.weapon_change_direction != Direction::None,
            },
        );

        let body = scene.physics.bodies.get_mut(self.body.into()).unwrap();

        let pivot = &scene.graph[self.pivot];

        let look_vector = pivot
            .look_vector()
            .try_normalize(std::f32::EPSILON)
            .unwrap_or(Vector3::z());

        let side_vector = pivot
            .side_vector()
            .try_normalize(std::f32::EPSILON)
            .unwrap_or(Vector3::x());

        let position = pivot.local_transform().position();

        let mut velocity = Vector3::default();

        if self.controller.walk_right {
            velocity -= side_vector;
        }
        if self.controller.walk_left {
            velocity += side_vector;
        }
        if self.controller.walk_forward {
            velocity += look_vector;
        }
        if self.controller.walk_backward {
            velocity -= look_vector;
        }

        let can_move = self.lower_body_machine.machine.active_state()
            != self.lower_body_machine.fall_state
            && self.lower_body_machine.machine.active_state() != self.lower_body_machine.land_state;

        let speed = if can_move {
            self.move_speed * time.delta
        } else {
            0.0
        };
        let velocity = velocity
            .try_normalize(std::f32::EPSILON)
            .and_then(|v| Some(v.scale(speed)))
            .unwrap_or(Vector3::default());

        let is_moving = velocity.norm_squared() > 0.0;

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
                new_y_vel = Some(3.0 * time.delta);
            }
        }

        while let Some(event) = scene
            .animations
            .get_mut(self.upper_body_machine.grab_animation)
            .pop_event()
        {
            if event.signal_id == UpperBodyMachine::GRAB_WEAPON_SIGNAL {
                match self.weapon_change_direction {
                    Direction::None => (),
                    Direction::Next => self.next_weapon(),
                    Direction::Previous => self.prev_weapon(),
                }

                self.weapon_change_direction = Direction::None;
            }
        }

        let quat_yaw = UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.controller.yaw);

        body.wake_up(true);
        body.set_angvel(Default::default(), true);
        if let Some(new_y_vel) = new_y_vel {
            body.set_linvel(
                Vector3::new(
                    velocity.x / time.delta,
                    new_y_vel / time.delta,
                    velocity.z / time.delta,
                ),
                true,
            );
        } else {
            body.set_linvel(
                Vector3::new(
                    velocity.x / time.delta,
                    body.linvel().y,
                    velocity.z / time.delta,
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

        if is_moving || self.controller.aim {
            // Since we have free camera while not moving, we have to sync rotation of pivot
            // with rotation of camera so character will start moving in look direction.
            let mut current_position = *body.position();
            current_position.rotation = quat_yaw;
            body.set_position(current_position, true);

            // Apply additional rotation to model - it will turn in front of walking direction.
            let angle: f32 = if self.controller.aim {
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
            } else {
                if self.controller.walk_left {
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
                } else {
                    if self.controller.walk_backward {
                        180.0
                    } else {
                        0.0
                    }
                }
            };

            self.model_yaw
                .set_target(angle.to_radians())
                .update(time.delta);

            if self.controller.aim {
                scene.graph[self.model]
                    .local_transform_mut()
                    .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0));

                let spine_transform = scene.graph[self.spine].local_transform_mut();
                spine_transform.set_rotation(
                    spine_transform.rotation()
                        * UnitQuaternion::from_axis_angle(
                            &Vector3::x_axis(),
                            self.spine_pitch.angle,
                        )
                        * UnitQuaternion::from_axis_angle(
                            &Vector3::y_axis(),
                            -(self.model_yaw.angle + 37.5f32.to_radians()),
                        ),
                );
                scene.graph[self.hips].local_transform_mut().set_rotation(
                    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.model_yaw.angle),
                );
            } else {
                scene.graph[self.model].local_transform_mut().set_rotation(
                    UnitQuaternion::from_axis_angle(&Vector3::y_axis(), self.model_yaw.angle),
                );

                scene.graph[self.spine].local_transform_mut().set_rotation(
                    UnitQuaternion::from_axis_angle(&Vector3::x_axis(), self.spine_pitch.angle)
                        * UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0),
                );
                scene.graph[self.hips]
                    .local_transform_mut()
                    .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), 0.0));
            }

            let walk_dir = if self.controller.aim && self.controller.walk_backward {
                -1.0
            } else {
                1.0
            };

            scene
                .animations
                .get_mut(self.lower_body_machine.walk_animation)
                .set_speed(walk_dir);
        }

        let ray_origin = scene.graph[self.camera_pivot].global_position();
        let ray_end = scene.graph[self.camera].global_position();
        let dir = (ray_end - ray_origin)
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_default()
            .scale(10.0);
        let ray = Ray {
            origin: ray_origin,
            dir,
        };
        let mut results = Vec::new();
        scene.physics.cast_ray(
            RayCastOptions {
                ray,
                max_len: ray.dir.norm(),
                groups: Default::default(),
                sort_results: true,
            },
            &mut results,
        );

        self.camera_offset = 0.8;

        for result in results {
            if result.collider != self.collider {
                self.camera_offset = result.toi.min(0.8);
                break;
            }
        }

        scene.graph[self.camera]
            .local_transform_mut()
            .set_position(Vector3::new(0.0, 0.0, -self.camera_offset));

        scene.graph[self.camera_pivot]
            .local_transform_mut()
            .set_rotation(quat_yaw)
            .set_position(position + velocity);

        // Rotate camera hinge - this will make camera move up and down while look at character
        // (well not exactly on character - on characters head)
        scene.graph[self.camera_hinge]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(
                &Vector3::x_axis(),
                self.controller.pitch,
            ));

        if has_ground_contact && self.controller.jump {
            // Rewind jump animation to beginning before jump.
            scene
                .animations
                .get_mut(self.lower_body_machine.jump_animation)
                .rewind();
            scene
                .animations
                .get_mut(self.upper_body_machine.jump_animation)
                .rewind();
        }

        if !has_ground_contact {
            scene
                .animations
                .get_mut(self.lower_body_machine.land_animation)
                .rewind();
            scene
                .animations
                .get_mut(self.upper_body_machine.land_animation)
                .rewind();
        }

        if let Some(current_weapon_handle) = self
            .character
            .weapons
            .get(self.character.current_weapon as usize)
        {
            let initial_velocity = *context
                .scene
                .physics
                .bodies
                .get(self.character.body.into())
                .unwrap()
                .linvel();
            if self.controller.shoot {
                self.character
                    .sender
                    .as_ref()
                    .unwrap()
                    .send(Message::ShootWeapon {
                        weapon: *current_weapon_handle,
                        initial_velocity,
                        direction: None,
                    })
                    .unwrap();
            }
        }
    }

    pub fn process_input_event(&mut self, event: &Event<()>, dt: f32, scene: &mut Scene) {
        let scheme = self.control_scheme.clone().unwrap();
        let scheme = scheme.read().unwrap();

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
                    let mouse_sens = scheme.mouse_sens * dt;
                    self.controller.yaw -= (delta.0 as f32) * mouse_sens;
                    self.controller.pitch = (self.controller.pitch + (delta.1 as f32) * mouse_sens)
                        .max(-90.0f32.to_radians())
                        .min(90.0f32.to_radians());
                    None
                }
                _ => None,
            },
            _ => None,
        };

        if let Some((button, state)) = button_state {
            if button == scheme.aim.button {
                self.controller.aim = state == ElementState::Pressed;
            } else if button == scheme.move_forward.button {
                self.controller.walk_forward = state == ElementState::Pressed;
            } else if button == scheme.move_backward.button {
                self.controller.walk_backward = state == ElementState::Pressed;
            } else if button == scheme.move_left.button {
                self.controller.walk_left = state == ElementState::Pressed;
            } else if button == scheme.move_right.button {
                self.controller.walk_right = state == ElementState::Pressed;
            } else if button == scheme.jump.button {
                self.controller.jump = state == ElementState::Pressed;
            } else if button == scheme.next_weapon.button {
                if state == ElementState::Pressed {
                    self.weapon_change_direction = Direction::Next;

                    scene
                        .animations
                        .get_mut(self.upper_body_machine.put_back_animation)
                        .rewind();

                    scene
                        .animations
                        .get_mut(self.upper_body_machine.grab_animation)
                        .rewind();
                }
            } else if button == scheme.prev_weapon.button {
                if state == ElementState::Pressed {
                    self.weapon_change_direction = Direction::Previous;

                    scene
                        .animations
                        .get_mut(self.upper_body_machine.put_back_animation)
                        .rewind();

                    scene
                        .animations
                        .get_mut(self.upper_body_machine.grab_animation)
                        .rewind();
                }
            } else if button == scheme.toss_grenade.button {
                self.controller.toss_grenade = state == ElementState::Pressed;
                if state == ElementState::Pressed {
                    scene
                        .animations
                        .get_mut(self.upper_body_machine.toss_grenade_animation)
                        .rewind();
                }
            } else if button == scheme.shoot.button {
                self.controller.shoot = state == ElementState::Pressed;
            }
        }
    }
}
