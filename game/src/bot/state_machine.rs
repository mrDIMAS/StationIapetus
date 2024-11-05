use crate::bot::MovementType;
use fyrox::graph::SceneGraph;
use fyrox::{
    core::pool::Handle,
    scene::{animation::absm::prelude::*, animation::prelude::*, graph::Graph, node::Node, Scene},
};

pub struct StateMachineInput {
    pub walk: bool,
    pub scream: bool,
    pub dead: bool,
    pub movement_speed_factor: f32,
    pub attack: bool,
    pub attack_animation_index: u32,
    pub aim: bool,
    pub badly_damaged: bool,
    pub movement_type: MovementType,
}

#[derive(Default, Debug, Clone)]
pub struct StateMachine {
    pub absm: Handle<Node>,
    pub aim_state: Handle<State>,
    pub attack_state: Handle<State>,
    pub threaten_state: Handle<State>,
    pub dead_state: Handle<State>,
    pub attack_animations: Vec<Handle<Animation>>,
}

impl StateMachine {
    pub const HIT_SIGNAL: &'static str = "Hit";
    pub const STEP_SIGNAL: &'static str = "Footstep";

    const LOWER_BODY_LAYER_INDEX: usize = 0;
    const UPPER_BODY_LAYER_INDEX: usize = 1;

    pub fn new(machine_handle: Handle<Node>, graph: &Graph) -> Option<Self> {
        let absm = graph.try_get_of_type::<AnimationBlendingStateMachine>(machine_handle)?;
        let machine = absm.machine();

        let (upper_body_layer_index, upper_body) = machine.find_layer_by_name_ref("UpperBody")?;
        assert_eq!(upper_body_layer_index, Self::UPPER_BODY_LAYER_INDEX);

        let attack_state = upper_body.find_state_by_name_ref("MeleeAttack")?.0;

        Some(Self {
            attack_state,
            absm: machine_handle,
            aim_state: upper_body.find_state_by_name_ref("Aim")?.0,
            threaten_state: upper_body.find_state_by_name_ref("Threaten")?.0,
            dead_state: upper_body.find_state_by_name_ref("Dead")?.0,
            attack_animations: upper_body
                .animations_of_state(attack_state)
                .collect::<Vec<_>>(),
        })
    }

    pub fn apply(&mut self, scene: &mut Scene, input: StateMachineInput) {
        scene
            .graph
            .try_get_mut_of_type::<AnimationBlendingStateMachine>(self.absm)
            .unwrap()
            .machine_mut()
            .get_value_mut_silent()
            .set_parameter("Attack", Parameter::Rule(input.attack))
            .set_parameter(
                "AttackAnimationIndex",
                Parameter::Index(input.attack_animation_index),
            )
            .set_parameter("Walk", Parameter::Rule(input.walk))
            .set_parameter("Threaten", Parameter::Rule(input.scream))
            .set_parameter("Aim", Parameter::Rule(input.aim))
            .set_parameter("Dead", Parameter::Rule(input.dead))
            .set_parameter("WasHit", Parameter::Rule(input.badly_damaged))
            .set_parameter("MovementType", Parameter::Index(input.movement_type as u32));
    }

    pub fn fetch_layer<'a>(&self, graph: &'a Graph, idx: usize) -> Option<&'a MachineLayer> {
        graph
            .try_get_of_type::<AnimationBlendingStateMachine>(self.absm)
            .and_then(|absm| absm.machine().layers().get(idx))
    }

    pub fn lower_body_layer<'a>(&self, graph: &'a Graph) -> Option<&'a MachineLayer> {
        self.fetch_layer(graph, Self::LOWER_BODY_LAYER_INDEX)
    }

    pub fn upper_body_layer<'a>(&self, graph: &'a Graph) -> Option<&'a MachineLayer> {
        self.fetch_layer(graph, Self::UPPER_BODY_LAYER_INDEX)
    }

    pub fn is_in_aim_state(&self, graph: &Graph) -> bool {
        self.upper_body_layer(graph)
            .map_or(false, |layer| layer.active_state() == self.aim_state)
    }
}
