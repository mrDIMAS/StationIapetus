use crate::{
    create_play_animation_state,
    player::{make_walk_state, WalkStateDefinition},
};
use rg3d::{
    animation::{
        machine::{Machine, Parameter, State, Transition},
        Animation, AnimationSignal,
    },
    core::{
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    scene::{node::Node, Scene},
};

#[derive(Default)]
pub struct LowerBodyMachine {
    pub machine: Machine,
    pub jump_animation: Handle<Animation>,
    pub walk_animation: Handle<Animation>,
    pub run_animation: Handle<Animation>,
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
        self.run_animation.visit("RunAnimation", visitor)?;
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
    pub is_walking: bool,
    pub is_jumping: bool,
    pub run_factor: f32,
    pub has_ground_contact: bool,
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

    const RUN_FACTOR: &'static str = "RunFactor";
    const WALK_FACTOR: &'static str = "WalkFactor";

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
            run_animation_resource,
        ) = rg3d::futures::join!(
            resource_manager.request_model("data/animations/agent_walk_rifle.fbx"),
            resource_manager.request_model("data/animations/agent_idle.fbx"),
            resource_manager.request_model("data/animations/agent_jump.fbx"),
            resource_manager.request_model("data/animations/agent_falling.fbx"),
            resource_manager.request_model("data/animations/agent_landing.fbx"),
            resource_manager.request_model("data/animations/agent_run_rifle.fbx"),
        );

        let (_, idle_state) = create_play_animation_state(
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

        let (_, fall_state) = create_play_animation_state(
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

        let WalkStateDefinition {
            walk_animation,
            state: walk_state,
            run_animation,
        } = make_walk_state(
            &mut machine,
            scene,
            model,
            walk_animation_resource.unwrap(),
            run_animation_resource.unwrap(),
            Self::WALK_FACTOR.to_owned(),
            Self::RUN_FACTOR.to_owned(),
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
            run_animation,
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
            .set_parameter(Self::WALK_FACTOR, Parameter::Weight(1.0 - input.run_factor))
            .set_parameter(Self::RUN_FACTOR, Parameter::Weight(input.run_factor))
            .evaluate_pose(&scene.animations, dt)
            .apply(&mut scene.graph);
    }
}
