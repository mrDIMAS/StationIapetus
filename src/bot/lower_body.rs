use crate::{
    bot::{clean_machine, BotDefinition},
    create_play_animation_state,
};
use rg3d::engine::resource_manager::MaterialSearchOptions;
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
use std::path::PathBuf;

#[derive(Default)]
pub struct LowerBodyMachine {
    pub machine: Machine,
    pub walk_animation: Handle<Animation>,
    pub dying_animation: Handle<Animation>,
    pub walk_state: Handle<State>,
}

#[derive(Debug)]
pub struct LowerBodyMachineInput {
    pub walk: bool,
    pub scream: bool,
    pub dead: bool,
}

impl Visit for LowerBodyMachine {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.machine.visit("Machine", visitor)?;
        self.walk_animation.visit("WalkAnimation", visitor)?;
        self.dying_animation.visit("DyingAnimation", visitor)?;
        self.walk_state.visit("WalkState", visitor)?;

        visitor.leave_region()
    }
}

impl LowerBodyMachine {
    pub const STEP_SIGNAL: u64 = 1;

    const IDLE_TO_WALK: &'static str = "IdleToWalk";
    const WALK_TO_IDLE: &'static str = "WalkToIdle";
    const IDLE_TO_SCREAM: &'static str = "IdleToScream";
    const SCREAM_TO_WALK: &'static str = "ScreamToWalk";
    const SCREAM_TO_IDLE: &'static str = "ScreamToIdle";
    const WALK_TO_DYING: &'static str = "WalkToDying";
    const IDLE_TO_DYING: &'static str = "IdleToDying";

    pub async fn new(
        resource_manager: ResourceManager,
        definition: &BotDefinition,
        model: Handle<Node>,
        scene: &mut Scene,
    ) -> Self {
        let (
            idle_animation_resource,
            walk_animation_resource,
            scream_animation_resource,
            dying_animation_resource,
        ) = rg3d::core::futures::join!(
            resource_manager.request_model(
                &definition.idle_animation,
                MaterialSearchOptions::MaterialsDirectory(PathBuf::from("data/textures"))
            ),
            resource_manager.request_model(
                &definition.walk_animation,
                MaterialSearchOptions::MaterialsDirectory(PathBuf::from("data/textures"))
            ),
            resource_manager.request_model(
                &definition.scream_animation,
                MaterialSearchOptions::MaterialsDirectory(PathBuf::from("data/textures"))
            ),
            resource_manager.request_model(
                &definition.dying_animation,
                MaterialSearchOptions::MaterialsDirectory(PathBuf::from("data/textures"))
            ),
        );

        let mut machine = Machine::new();

        let (_, idle_state) = create_play_animation_state(
            idle_animation_resource.unwrap(),
            "Idle",
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

        let (_, scream_state) = create_play_animation_state(
            scream_animation_resource.unwrap(),
            "Scream",
            &mut machine,
            scene,
            model,
        );

        let (dying_animation, dying_state) = create_play_animation_state(
            dying_animation_resource.unwrap(),
            "Dying",
            &mut machine,
            scene,
            model,
        );

        scene
            .animations
            .get_mut(dying_animation)
            .set_loop(false)
            .set_enabled(false)
            .set_speed(1.0);

        scene.animations[walk_animation]
            .add_signal(AnimationSignal::new(Self::STEP_SIGNAL, 0.3))
            .add_signal(AnimationSignal::new(Self::STEP_SIGNAL, 0.6));

        machine.add_transition(Transition::new(
            "Idle->Walk",
            idle_state,
            walk_state,
            0.2,
            Self::IDLE_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "Walk->Idle",
            walk_state,
            idle_state,
            0.2,
            Self::WALK_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "Idle->Scream",
            idle_state,
            scream_state,
            0.2,
            Self::IDLE_TO_SCREAM,
        ));
        machine.add_transition(Transition::new(
            "Scream->Walk",
            scream_state,
            walk_state,
            0.2,
            Self::SCREAM_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "Scream->Idle",
            scream_state,
            idle_state,
            0.2,
            Self::SCREAM_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "Walk->Dying",
            walk_state,
            dying_state,
            0.2,
            Self::WALK_TO_DYING,
        ));
        machine.add_transition(Transition::new(
            "Idle->Dying",
            idle_state,
            dying_state,
            0.2,
            Self::IDLE_TO_DYING,
        ));

        machine.set_entry_state(idle_state);

        Self {
            machine,
            walk_animation,
            dying_animation,
            walk_state,
        }
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        clean_machine(&self.machine, scene)
    }

    pub fn set_walk_animation_speed(&self, scene: &mut Scene, speed: f32) {
        scene.animations[self.walk_animation].set_speed(speed);
    }

    pub fn apply(&mut self, scene: &mut Scene, dt: f32, input: LowerBodyMachineInput) {
        self.machine
            .set_parameter(Self::IDLE_TO_WALK, Parameter::Rule(input.walk))
            .set_parameter(Self::WALK_TO_IDLE, Parameter::Rule(!input.walk))
            .set_parameter(Self::IDLE_TO_SCREAM, Parameter::Rule(input.scream))
            .set_parameter(Self::SCREAM_TO_WALK, Parameter::Rule(!input.scream))
            .set_parameter(Self::SCREAM_TO_IDLE, Parameter::Rule(!input.scream))
            .set_parameter(Self::WALK_TO_DYING, Parameter::Rule(input.dead))
            .set_parameter(Self::IDLE_TO_DYING, Parameter::Rule(input.dead))
            .evaluate_pose(&scene.animations, dt)
            .apply(&mut scene.graph);
    }

    pub fn is_walking(&self) -> bool {
        let active_transition = self.machine.active_transition();
        self.machine.active_state() == self.walk_state
            || (active_transition.is_some()
                && self.machine.transitions().borrow(active_transition).dest() == self.walk_state)
    }
}
