use crate::{bot::BotDefinition, utils, utils::create_play_animation_state};
use fyrox::animation::RootMotionSettings;
use fyrox::resource::model::Model;
use fyrox::{
    animation::{
        machine::{Machine, Parameter, State, Transition},
        Animation, AnimationSignal,
    },
    asset::manager::ResourceManager,
    core::{
        pool::Handle,
        uuid::{uuid, Uuid},
        visitor::{Visit, VisitResult, Visitor},
    },
    scene::{node::Node, Scene},
};

#[derive(Default, Visit, Clone, Debug)]
pub struct LowerBodyMachine {
    pub machine: Machine,
    pub walk_animation: Handle<Animation>,
    pub dying_animation: Handle<Animation>,
    pub scream_animation: Handle<Animation>,
    pub walk_state: Handle<State>,
}

#[derive(Debug)]
pub struct LowerBodyMachineInput {
    pub walk: bool,
    pub scream: bool,
    pub dead: bool,
    pub movement_speed_factor: f32,
}

impl LowerBodyMachine {
    pub const STEP_SIGNAL: Uuid = uuid!("f3d77f3b-a642-4297-ab04-7bc700056749");

    const IDLE_TO_WALK: &'static str = "IdleToWalk";
    const WALK_TO_IDLE: &'static str = "WalkToIdle";
    const IDLE_TO_SCREAM: &'static str = "IdleToScream";
    const WALK_TO_SCREAM: &'static str = "WalkToScream";
    const SCREAM_TO_WALK: &'static str = "ScreamToWalk";
    const SCREAM_TO_IDLE: &'static str = "ScreamToIdle";
    const WALK_TO_DYING: &'static str = "WalkToDying";
    const IDLE_TO_DYING: &'static str = "IdleToDying";

    pub async fn new(
        resource_manager: ResourceManager,
        definition: &BotDefinition,
        model: Handle<Node>,
        animation_player: Handle<Node>,
        hips: Handle<Node>,
        scene: &mut Scene,
    ) -> Self {
        let (
            idle_animation_resource,
            walk_animation_resource,
            scream_animation_resource,
            dying_animation_resource,
        ) = fyrox::core::futures::join!(
            resource_manager.request::<Model, _>(&definition.idle_animation,),
            resource_manager.request::<Model, _>(&definition.walk_animation,),
            resource_manager.request::<Model, _>(&definition.scream_animation,),
            resource_manager.request::<Model, _>(&definition.dying_animation,),
        );

        let mut machine = Machine::new();

        let root_layer = machine.layers_mut().first_mut().unwrap();

        let (_, idle_state) = create_play_animation_state(
            idle_animation_resource.unwrap(),
            "Idle",
            root_layer,
            scene,
            model,
            animation_player,
        );

        let (walk_animation, walk_state) = create_play_animation_state(
            walk_animation_resource.unwrap(),
            "Walk",
            root_layer,
            scene,
            model,
            animation_player,
        );

        let (scream_animation, scream_state) = create_play_animation_state(
            scream_animation_resource.unwrap(),
            "Scream",
            root_layer,
            scene,
            model,
            animation_player,
        );

        let (dying_animation, dying_state) = create_play_animation_state(
            dying_animation_resource.unwrap(),
            "Dying",
            root_layer,
            scene,
            model,
            animation_player,
        );

        let animations_container =
            utils::fetch_animation_container_mut(&mut scene.graph, animation_player);

        animations_container
            .get_mut(scream_animation)
            .set_loop(false)
            .set_enabled(false)
            .set_speed(1.0);

        animations_container
            .get_mut(dying_animation)
            .set_loop(false)
            .set_enabled(false)
            .set_speed(1.0);

        let walk_animation_ref = &mut animations_container[walk_animation];

        walk_animation_ref.set_root_motion_settings(Some(RootMotionSettings {
            node: hips,
            ignore_x_movement: false,
            ignore_y_movement: true,
            ignore_z_movement: false,
            ignore_rotations: true,
        }));

        walk_animation_ref
            .add_signal(AnimationSignal {
                id: Self::STEP_SIGNAL,
                name: "Step1".to_string(),
                time: 0.3,
                enabled: true,
            })
            .add_signal(AnimationSignal {
                id: Self::STEP_SIGNAL,
                name: "Step2".to_string(),
                time: 0.6,
                enabled: true,
            });

        root_layer.add_transition(Transition::new(
            "Idle->Walk",
            idle_state,
            walk_state,
            0.2,
            Self::IDLE_TO_WALK,
        ));
        root_layer.add_transition(Transition::new(
            "Walk->Idle",
            walk_state,
            idle_state,
            0.2,
            Self::WALK_TO_IDLE,
        ));
        root_layer.add_transition(Transition::new(
            "Idle->Scream",
            idle_state,
            scream_state,
            0.2,
            Self::IDLE_TO_SCREAM,
        ));
        root_layer.add_transition(Transition::new(
            "Walk->Scream",
            walk_state,
            scream_state,
            0.2,
            Self::WALK_TO_SCREAM,
        ));
        root_layer.add_transition(Transition::new(
            "Scream->Walk",
            scream_state,
            walk_state,
            0.2,
            Self::SCREAM_TO_WALK,
        ));
        root_layer.add_transition(Transition::new(
            "Scream->Idle",
            scream_state,
            idle_state,
            0.2,
            Self::SCREAM_TO_IDLE,
        ));
        root_layer.add_transition(Transition::new(
            "Walk->Dying",
            walk_state,
            dying_state,
            0.2,
            Self::WALK_TO_DYING,
        ));
        root_layer.add_transition(Transition::new(
            "Idle->Dying",
            idle_state,
            dying_state,
            0.2,
            Self::IDLE_TO_DYING,
        ));

        root_layer.set_entry_state(idle_state);

        Self {
            scream_animation,
            machine,
            walk_animation,
            dying_animation,
            walk_state,
        }
    }

    pub fn apply(
        &mut self,
        scene: &mut Scene,
        dt: f32,
        input: LowerBodyMachineInput,
        animation_player: Handle<Node>,
    ) {
        let animations_container =
            utils::fetch_animation_container_ref(&scene.graph, animation_player);

        self.machine
            .set_parameter(Self::IDLE_TO_WALK, Parameter::Rule(input.walk))
            .set_parameter(Self::WALK_TO_IDLE, Parameter::Rule(!input.walk))
            .set_parameter(Self::IDLE_TO_SCREAM, Parameter::Rule(input.scream))
            .set_parameter(Self::WALK_TO_SCREAM, Parameter::Rule(input.scream))
            .set_parameter(Self::SCREAM_TO_WALK, Parameter::Rule(!input.scream))
            .set_parameter(Self::SCREAM_TO_IDLE, Parameter::Rule(!input.scream))
            .set_parameter(Self::WALK_TO_DYING, Parameter::Rule(input.dead))
            .set_parameter(Self::IDLE_TO_DYING, Parameter::Rule(input.dead))
            .evaluate_pose(animations_container, dt)
            .apply(&mut scene.graph);
    }

    pub fn is_walking(&self) -> bool {
        let root_layer = self.machine.layers().first().unwrap();
        let active_transition = root_layer.active_transition();
        root_layer.active_state() == self.walk_state
            || (active_transition.is_some()
                && root_layer.transitions().borrow(active_transition).dest() == self.walk_state)
    }
}
