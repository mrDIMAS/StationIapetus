use crate::{character::Character, sound::SoundManager, utils};
use fyrox::{
    animation::{
        machine::MachineLayer,
        machine::Parameter,
        machine::{State, Transition},
        Animation,
    },
    core::{algebra::Vector3, pool::Handle},
    scene::{
        animation::{absm::AnimationBlendingStateMachine, AnimationPlayer},
        graph::Graph,
        node::Node,
        Scene,
    },
};

#[derive(Eq, PartialEq, Copy, Clone)]
#[repr(u32)]
pub enum CombatWeaponKind {
    Pistol = 0,
    Rifle = 1,
}

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
    pub machine: Handle<Node>,
    pub scene: &'a mut Scene,
}

#[derive(Default, Debug, Clone)]
pub struct StateMachine {
    pub machine_handle: Handle<Node>,
    pub jump_animation: Handle<Animation>,
    pub walk_animation: Handle<Animation>,
    pub run_animation: Handle<Animation>,
    pub land_animation: Handle<Animation>,
    pub dying_animation: Handle<Animation>,
    pub hit_reaction_pistol_animation: Handle<Animation>,
    pub hit_reaction_rifle_animation: Handle<Animation>,
    pub walk_state: Handle<State>,
    pub jump_state: Handle<State>,
    pub fall_state: Handle<State>,
    pub land_state: Handle<State>,
    pub walk_to_jump: Handle<Transition>,
    pub idle_to_jump: Handle<Transition>,
    pub aim_state: Handle<State>,
    pub toss_grenade_state: Handle<State>,
    pub put_back_state: Handle<State>,
    pub toss_grenade_animation: Handle<Animation>,
    pub put_back_animation: Handle<Animation>,
    pub grab_animation: Handle<Animation>,
}

impl StateMachine {
    pub const FOOTSTEP_SIGNAL: &'static str = "Footstep";
    pub const JUMP_SIGNAL: &'static str = "Jump";
    pub const GRAB_WEAPON_SIGNAL: &'static str = "Grab";
    pub const PUT_BACK_WEAPON_END_SIGNAL: &'static str = "PutBack";
    pub const TOSS_GRENADE_SIGNAL: &'static str = "TossGrenade";

    pub fn new(machine_handle: Handle<Node>, graph: &Graph) -> Option<Self> {
        let absm = graph.try_get_of_type::<AnimationBlendingStateMachine>(machine_handle)?;

        let animation_player = graph.try_get_of_type::<AnimationPlayer>(absm.animation_player())?;
        let animations = animation_player.animations();
        let machine = absm.machine();

        let lower_body = machine.find_layer_by_name_ref("LowerBody")?.1;
        let upper_body = machine.find_layer_by_name_ref("UpperBody")?.1;

        Some(Self {
            machine_handle,
            jump_animation: animations.find_by_name_ref("agent_jump")?.0,
            walk_animation: animations.find_by_name_ref("agent_walk_rifle")?.0,
            run_animation: animations.find_by_name_ref("agent_run_rifle")?.0,
            land_animation: animations.find_by_name_ref("agent_landing")?.0,
            dying_animation: animations.find_by_name_ref("agent_dying")?.0,
            hit_reaction_pistol_animation: animations
                .find_by_name_ref("agent_hit_reaction_pistol")?
                .0,
            hit_reaction_rifle_animation: animations
                .find_by_name_ref("agent_hit_reaction_rifle")?
                .0,
            walk_state: lower_body.find_state_by_name_ref("Walk")?.0,
            jump_state: lower_body.find_state_by_name_ref("Jump")?.0,
            fall_state: lower_body.find_state_by_name_ref("Fall")?.0,
            land_state: lower_body.find_state_by_name_ref("Land")?.0,
            walk_to_jump: lower_body.find_transition_by_name_ref("WalkToJump")?.0,
            idle_to_jump: lower_body.find_transition_by_name_ref("IdleToJump")?.0,
            aim_state: upper_body.find_state_by_name_ref("Aim")?.0,
            toss_grenade_state: upper_body.find_state_by_name_ref("TossGrenade")?.0,
            put_back_state: upper_body.find_state_by_name_ref("PutBack")?.0,
            toss_grenade_animation: animations.find_by_name_ref("agent_toss_grenade")?.0,
            put_back_animation: animations.find_by_name_ref("agent_put_back")?.0,
            grab_animation: animations.find_by_name_ref("agent_grab")?.0,
        })
    }

    pub fn fetch_layer<'a>(&self, graph: &'a Graph, name: &str) -> Option<&'a MachineLayer> {
        graph
            .try_get_of_type::<AnimationBlendingStateMachine>(self.machine_handle)
            .and_then(|absm| absm.machine().find_layer_by_name_ref(name).map(|(_, l)| l))
    }

    pub fn lower_body_layer<'a>(&self, graph: &'a Graph) -> Option<&'a MachineLayer> {
        self.fetch_layer(graph, "LowerBody")
    }

    pub fn upper_body_layer<'a>(&self, graph: &'a Graph) -> Option<&'a MachineLayer> {
        self.fetch_layer(graph, "UpperBody")
    }

    pub fn handle_animation_events(
        &self,
        character: &Character,
        sound_manager: &SoundManager,
        position: Vector3<f32>,
        scene: &mut Scene,
        is_walking: bool,
        run_factor: f32,
        has_ground_contact: bool,
    ) {
        let begin = position + Vector3::new(0.0, 0.5, 0.0);

        if let Some(absm) = scene
            .graph
            .try_get_of_type::<AnimationBlendingStateMachine>(self.machine_handle)
        {
            if let Some(animation_player) = scene
                .graph
                .try_get_of_type::<AnimationPlayer>(absm.animation_player())
            {
                let animations_container = animation_player.animations();

                let mut walk_events = animations_container.get(self.walk_animation).events();
                let mut run_events = animations_container.get(self.run_animation).events();
                let mut land_events = animations_container.get(self.land_animation).events();

                while let Some((walking, evt)) = walk_events
                    .pop_front()
                    .map(|e| (true, e))
                    .or_else(|| run_events.pop_front().map(|e| (false, e)))
                {
                    if is_walking
                        && has_ground_contact
                        && evt.name == Self::FOOTSTEP_SIGNAL
                        && run_factor < 0.5
                        && walking
                        || run_factor >= 0.5 && !walking
                    {
                        character.footstep_ray_check(begin, scene, sound_manager);
                    }
                }

                while let Some(evt) = land_events.pop_front() {
                    if evt.name == Self::FOOTSTEP_SIGNAL {
                        character.footstep_ray_check(begin, scene, sound_manager);
                    }
                }
            }
        }
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
            machine,
            scene,
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

        let jump_animation_ended = animations_container.get(self.jump_animation).has_ended();
        let land_animation_ended = animations_container.get(self.land_animation).has_ended();
        let put_back_animation_ended = animations_container
            .get(self.put_back_animation)
            .has_ended();
        let grab_animation_ended = animations_container.get(self.grab_animation).has_ended();

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
            .set_parameter("GrabWeapon", Parameter::Rule(put_back_animation_ended))
            .set_parameter("RemoveWeapon", Parameter::Rule(change_weapon))
            .set_parameter("WeaponChanged", Parameter::Rule(grab_animation_ended));
    }

    pub fn hit_reaction_animations(&self) -> [Handle<Animation>; 2] {
        [
            self.hit_reaction_rifle_animation,
            self.hit_reaction_pistol_animation,
        ]
    }

    pub fn is_stunned(&self, scene: &Scene, animation_player: Handle<Node>) -> bool {
        let animations_container =
            utils::fetch_animation_container_ref(&scene.graph, animation_player);

        let hr_animation = &animations_container[self.hit_reaction_rifle_animation];
        !hr_animation.has_ended() && hr_animation.is_enabled()
    }
}
