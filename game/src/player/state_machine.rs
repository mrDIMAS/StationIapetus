use crate::{utils, weapon::CombatWeaponKind};
use fyrox::graph::SceneGraph;
use fyrox::{
    core::{algebra::Vector2, pool::Handle},
    scene::{
        animation::{absm::prelude::*, prelude::*},
        graph::Graph,
        node::Node,
        Scene,
    },
};

pub struct StateMachineInput<'a> {
    pub is_walking: bool,
    pub is_jumping: bool,
    pub run_factor: f32,
    pub has_ground_contact: bool,
    pub is_aiming: bool,
    pub toss_grenade: bool,
    pub weapon_kind: CombatWeaponKind,
    pub change_weapon: bool,
    pub is_dead: bool,
    pub should_be_stunned: bool,
    pub melee_attack: bool,
    pub machine: Handle<Node>,
    pub scene: &'a mut Scene,
    pub local_velocity: Vector2<f32>,
    pub hit_something: bool,
}

#[derive(Default, Debug, Clone)]
pub struct StateMachine {
    pub machine_handle: Handle<Node>,
    pub jump_animation: Handle<Animation>,
    pub land_animation: Handle<Animation>,
    pub hit_reaction_pistol_animation: Handle<Animation>,
    pub hit_reaction_rifle_animation: Handle<Animation>,
    pub fall_state: Handle<State>,
    pub land_state: Handle<State>,
    pub aim_state: Handle<State>,
    pub grab_animation: Handle<Animation>,
}

impl StateMachine {
    pub const FOOTSTEP_SIGNAL: &'static str = "Footstep";
    pub const GRAB_WEAPON_SIGNAL: &'static str = "Grab";
    pub const TOSS_GRENADE_SIGNAL: &'static str = "TossGrenade";
    pub const HIT_STARTED_SIGNAL: &'static str = "HitStarted";
    pub const HIT_ENDED_SIGNAL: &'static str = "HitEnded";

    const LOWER_BODY_LAYER_INDEX: usize = 0;
    const UPPER_BODY_LAYER_INDEX: usize = 1;

    pub fn new(machine_handle: Handle<Node>, graph: &Graph) -> Option<Self> {
        let absm = graph.try_get_of_type::<AnimationBlendingStateMachine>(machine_handle)?;

        let animation_player = graph.try_get_of_type::<AnimationPlayer>(absm.animation_player())?;
        let animations = animation_player.animations();
        let machine = absm.machine();

        let (lower_body_layer_index, lower_body) = machine.find_layer_by_name_ref("LowerBody")?;
        assert_eq!(lower_body_layer_index, Self::LOWER_BODY_LAYER_INDEX);
        let (upper_body_layer_index, upper_body) = machine.find_layer_by_name_ref("UpperBody")?;
        assert_eq!(upper_body_layer_index, Self::UPPER_BODY_LAYER_INDEX);

        Some(Self {
            machine_handle,
            jump_animation: animations.find_by_name_ref("agent_jump")?.0,
            land_animation: animations.find_by_name_ref("agent_landing")?.0,
            hit_reaction_pistol_animation: animations
                .find_by_name_ref("agent_hit_reaction_pistol")?
                .0,
            hit_reaction_rifle_animation: animations
                .find_by_name_ref("agent_hit_reaction_rifle")?
                .0,
            fall_state: lower_body.find_state_by_name_ref("Fall")?.0,
            land_state: lower_body.find_state_by_name_ref("Land")?.0,
            aim_state: upper_body.find_state_by_name_ref("Aim")?.0,
            grab_animation: animations.find_by_name_ref("agent_grab")?.0,
        })
    }

    pub fn fetch_layer<'a>(&self, graph: &'a Graph, idx: usize) -> Option<&'a MachineLayer> {
        graph
            .try_get_of_type::<AnimationBlendingStateMachine>(self.machine_handle)
            .and_then(|absm| absm.machine().layers().get(idx))
    }

    pub fn lower_body_layer<'a>(&self, graph: &'a Graph) -> Option<&'a MachineLayer> {
        self.fetch_layer(graph, Self::LOWER_BODY_LAYER_INDEX)
    }

    pub fn upper_body_layer<'a>(&self, graph: &'a Graph) -> Option<&'a MachineLayer> {
        self.fetch_layer(graph, Self::UPPER_BODY_LAYER_INDEX)
    }

    pub fn fetch_layer_mut<'a>(
        &self,
        graph: &'a mut Graph,
        idx: usize,
    ) -> Option<&'a mut MachineLayer> {
        graph
            .try_get_mut_of_type::<AnimationBlendingStateMachine>(self.machine_handle)
            .and_then(|absm| {
                absm.machine_mut()
                    .get_value_mut_silent()
                    .layers_mut()
                    .get_mut(idx)
            })
    }

    #[allow(dead_code)]
    pub fn lower_body_layer_mut<'a>(&self, graph: &'a mut Graph) -> Option<&'a mut MachineLayer> {
        self.fetch_layer_mut(graph, Self::LOWER_BODY_LAYER_INDEX)
    }

    pub fn upper_body_layer_mut<'a>(&self, graph: &'a mut Graph) -> Option<&'a mut MachineLayer> {
        self.fetch_layer_mut(graph, Self::UPPER_BODY_LAYER_INDEX)
    }

    pub fn apply(&mut self, input: StateMachineInput) {
        let StateMachineInput {
            is_walking,
            is_jumping,
            run_factor,
            has_ground_contact,
            is_aiming,
            toss_grenade,
            weapon_kind,
            change_weapon,
            is_dead,
            should_be_stunned,
            melee_attack,
            machine,
            scene,
            local_velocity,
            hit_something,
        } = input;

        let animation_player = scene
            .graph
            .try_get_of_type::<AnimationBlendingStateMachine>(machine)
            .unwrap()
            .animation_player();

        let animations_container =
            utils::fetch_animation_container_ref(&scene.graph, animation_player);

        let current_hit_reaction_animation = match weapon_kind {
            CombatWeaponKind::Rifle => self.hit_reaction_rifle_animation,
            CombatWeaponKind::Pistol => self.hit_reaction_pistol_animation,
        };

        let recovered = !input.should_be_stunned
            && animations_container[current_hit_reaction_animation].has_ended();

        let land_animation_ended = animations_container.get(self.land_animation).has_ended();

        scene
            .graph
            .try_get_mut_of_type::<AnimationBlendingStateMachine>(machine)
            .unwrap()
            .machine_mut()
            .get_value_mut_silent()
            // Update parameters which will be used by transitions.
            .set_parameter("Walk", Parameter::Rule(is_walking))
            .set_parameter("Jump", Parameter::Rule(is_jumping))
            .set_parameter(
                "Landed",
                Parameter::Rule(has_ground_contact && land_animation_ended),
            )
            .set_parameter("WeaponKind", Parameter::Index(weapon_kind as u32))
            .set_parameter("HasGroundContact", Parameter::Rule(has_ground_contact))
            .set_parameter("Dead", Parameter::Rule(is_dead))
            .set_parameter("Aim", Parameter::Rule(is_aiming))
            .set_parameter("WalkFactor", Parameter::Weight(1.0 - run_factor))
            .set_parameter("RunFactor", Parameter::Weight(run_factor))
            .set_parameter("TossGrenade", Parameter::Rule(toss_grenade))
            .set_parameter("ReactToHit", Parameter::Rule(should_be_stunned))
            .set_parameter("RemoveWeapon", Parameter::Rule(change_weapon))
            .set_parameter("Recovered", Parameter::Rule(recovered))
            .set_parameter("Velocity", Parameter::SamplingPoint(local_velocity))
            .set_parameter("HitSomething", Parameter::Rule(dbg!(hit_something)))
            .set_parameter("MeleeAttack", Parameter::Rule(melee_attack));
    }

    pub fn is_stunned(&self, scene: &Scene, animation_player: Handle<Node>) -> bool {
        let animations_container =
            utils::fetch_animation_container_ref(&scene.graph, animation_player);

        let hr_animation = &animations_container[self.hit_reaction_rifle_animation];
        !hr_animation.has_ended() && hr_animation.is_enabled()
    }
}
