use crate::{
    bot::{clean_machine, BotDefinition},
    create_play_animation_state, GameTime,
};
use rg3d::animation::machine::{Machine, Parameter, State, Transition};
use rg3d::{
    animation::{Animation, AnimationSignal},
    core::{
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    scene::{node::Node, Scene},
};

#[derive(Default)]
pub struct UpperBodyMachine {
    pub machine: Machine,
    pub attack_animation: Handle<Animation>,
    pub aim_state: Handle<State>,
    pub dying_animation: Handle<Animation>,
}

#[derive(Debug)]
pub struct UpperBodyMachineInput {
    pub attack: bool,
    pub walk: bool,
    pub scream: bool,
    pub dead: bool,
    pub aim: bool,
}

impl UpperBodyMachine {
    pub const HIT_SIGNAL: u64 = 1;

    const ATTACK_TO_IDLE: &'static str = "AttackToIdle";
    const ATTACK_TO_WALK: &'static str = "AttackToWalk";
    const IDLE_TO_ATTACK: &'static str = "IdleToAttack";
    const WALK_TO_ATTACK: &'static str = "WalkToAttack";
    const IDLE_TO_WALK: &'static str = "IdleToWalk";
    const WALK_TO_IDLE: &'static str = "WalkToIdle";
    const IDLE_TO_SCREAM: &'static str = "IdleToScream";
    const SCREAM_TO_WALK: &'static str = "ScreamToWalk";
    const SCREAM_TO_IDLE: &'static str = "ScreamToIdle";
    const IDLE_TO_AIM: &'static str = "IdleToAim";
    const AIM_TO_IDLE: &'static str = "AimToIdle";
    const AIM_TO_WALK: &'static str = "AimToWalk";
    const ATTACK_TO_DYING: &'static str = "AttackToDying";
    const WALK_TO_DYING: &'static str = "WalkToDying";
    const IDLE_TO_DYING: &'static str = "IdleToDying";

    pub async fn new(
        resource_manager: ResourceManager,
        definition: &BotDefinition,
        model: Handle<Node>,
        scene: &mut Scene,
        attack_timestamp: f32,
    ) -> Self {
        let (
            idle_animation_resource,
            walk_animation_resource,
            scream_animation_resource,
            attack_animation_resource,
            dying_animation_resource,
        ) = rg3d::futures::join!(
            resource_manager.request_model(definition.idle_animation),
            resource_manager.request_model(definition.walk_animation),
            resource_manager.request_model(definition.scream_animation),
            resource_manager.request_model(definition.attack_animation),
            resource_manager.request_model(definition.dying_animation),
        );

        let aim_animation_resource =
            if definition.can_use_weapons && !definition.attack_animation.is_empty() {
                resource_manager
                    .request_model(definition.aim_animation)
                    .await
                    .ok()
            } else {
                None
            };

        let mut machine = Machine::new();

        let (aim_animation, aim_state) = if let Some(aim_animation_resource) =
            aim_animation_resource.clone()
        {
            create_play_animation_state(aim_animation_resource, "Aim", &mut machine, scene, model)
        } else {
            (Handle::NONE, Handle::NONE)
        };

        let (idle_animation, idle_state) = create_play_animation_state(
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

        let (scream_animation, scream_state) = create_play_animation_state(
            scream_animation_resource.unwrap(),
            "Scream",
            &mut machine,
            scene,
            model,
        );

        let (attack_animation, attack_state) = create_play_animation_state(
            attack_animation_resource.unwrap(),
            "Attack",
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
            .get_mut(attack_animation)
            .add_signal(AnimationSignal::new(Self::HIT_SIGNAL, attack_timestamp));

        scene
            .animations
            .get_mut(dying_animation)
            .set_loop(false)
            .set_enabled(false);

        scene
            .animations
            .get_mut(attack_animation)
            .set_loop(false)
            .set_enabled(false);

        for leg_name in &[definition.left_leg_name, definition.right_leg_name] {
            let leg_node = scene.graph.find_by_name(model, leg_name);

            for &animation in &[
                idle_animation,
                walk_animation,
                aim_animation,
                scream_animation,
                dying_animation,
                attack_animation,
            ] {
                // Some animations may be missing for some kinds of bots.
                if animation.is_some() {
                    scene.animations.get_mut(animation).set_tracks_enabled_from(
                        leg_node,
                        false,
                        &scene.graph,
                    )
                }
            }
        }

        machine.add_transition(Transition::new(
            "Attack->Idle",
            attack_state,
            idle_state,
            0.2,
            Self::ATTACK_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "Attack->Walk",
            attack_state,
            walk_state,
            0.2,
            Self::ATTACK_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "Idle->Attack",
            idle_state,
            attack_state,
            0.2,
            Self::IDLE_TO_ATTACK,
        ));
        machine.add_transition(Transition::new(
            "Walk->Attack",
            walk_state,
            attack_state,
            0.2,
            Self::WALK_TO_ATTACK,
        ));
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
        if aim_animation_resource.is_some() {
            machine.add_transition(Transition::new(
                "Idle->Aim",
                idle_state,
                aim_state,
                0.2,
                Self::IDLE_TO_AIM,
            ));
            machine.add_transition(Transition::new(
                "Aim->Idle",
                aim_state,
                idle_state,
                0.2,
                Self::AIM_TO_IDLE,
            ));
            machine.add_transition(Transition::new(
                "Aim->Walk",
                aim_state,
                walk_state,
                0.2,
                Self::AIM_TO_WALK,
            ));
        }
        machine.add_transition(Transition::new(
            "Attack->Dying",
            attack_state,
            dying_state,
            0.2,
            Self::ATTACK_TO_DYING,
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
            attack_animation,
            aim_state,
            dying_animation,
        }
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        clean_machine(&self.machine, scene)
    }

    pub fn apply(&mut self, scene: &mut Scene, time: GameTime, input: UpperBodyMachineInput) {
        self.machine
            .set_parameter(
                Self::ATTACK_TO_IDLE,
                Parameter::Rule(
                    !input.walk && scene.animations.get(self.attack_animation).has_ended(),
                ),
            )
            .set_parameter(
                Self::ATTACK_TO_WALK,
                Parameter::Rule(
                    input.walk && scene.animations.get(self.attack_animation).has_ended(),
                ),
            )
            .set_parameter(Self::IDLE_TO_ATTACK, Parameter::Rule(input.attack))
            .set_parameter(Self::WALK_TO_ATTACK, Parameter::Rule(input.attack))
            .set_parameter(Self::IDLE_TO_WALK, Parameter::Rule(input.walk))
            .set_parameter(Self::WALK_TO_IDLE, Parameter::Rule(!input.walk))
            .set_parameter(Self::IDLE_TO_SCREAM, Parameter::Rule(input.scream))
            .set_parameter(Self::SCREAM_TO_WALK, Parameter::Rule(!input.scream))
            .set_parameter(Self::SCREAM_TO_IDLE, Parameter::Rule(!input.scream))
            .set_parameter(Self::IDLE_TO_AIM, Parameter::Rule(input.aim))
            .set_parameter(Self::AIM_TO_IDLE, Parameter::Rule(!input.aim))
            .set_parameter(Self::AIM_TO_WALK, Parameter::Rule(input.walk && !input.aim))
            .set_parameter(Self::ATTACK_TO_DYING, Parameter::Rule(input.dead))
            .set_parameter(Self::WALK_TO_DYING, Parameter::Rule(input.dead))
            .set_parameter(Self::IDLE_TO_DYING, Parameter::Rule(input.dead))
            .evaluate_pose(&scene.animations, time.delta)
            .apply(&mut scene.graph);
    }
}

impl Visit for UpperBodyMachine {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.machine.visit("Machine", visitor)?;
        self.attack_animation.visit("AttackAnimation", visitor)?;
        self.dying_animation.visit("DyingAnimation", visitor)?;
        self.aim_state.visit("AimState", visitor)?;

        visitor.leave_region()
    }
}
