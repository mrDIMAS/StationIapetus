use crate::{
    bot::{clean_machine, BotDefinition},
    create_play_animation_state, GameTime, ModelMap,
};
use rg3d::{
    animation::{
        machine::{
            blend_nodes::IndexedBlendInput, Machine, Parameter, PoseNode, State, Transition,
        },
        Animation, AnimationSignal, PoseEvaluationFlags,
    },
    core::{
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    resource::model::Model,
    scene::{node::Node, Scene},
};

#[derive(Default)]
pub struct UpperBodyMachine {
    pub machine: Machine,
    pub attack_animations: Vec<Handle<Animation>>,
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
    pub attack_animation_index: u32,
}

pub fn make_attack_state(
    machine: &mut Machine,
    scene: &mut Scene,
    model: Handle<Node>,
    index_parameter: String,
    attack_animation_resources: Vec<(Model, f32)>,
    hips: Handle<Node>,
) -> (Handle<State>, Vec<Handle<Animation>>) {
    let animations = attack_animation_resources
        .into_iter()
        .map(|(resource, timestamp)| {
            let animation = *resource.retarget_animations(model, scene).get(0).unwrap();
            scene.animations[animation]
                .set_enabled(false)
                .set_loop(false)
                .add_signal(AnimationSignal::new(
                    UpperBodyMachine::HIT_SIGNAL,
                    timestamp,
                ))
                .set_speed(1.35)
                .track_of_mut(hips)
                .unwrap()
                .set_flags(PoseEvaluationFlags {
                    ignore_position: false,
                    ignore_rotation: true,
                    ignore_scale: false,
                });
            animation
        })
        .collect::<Vec<_>>();

    let poses = animations
        .iter()
        .map(|&animation| IndexedBlendInput {
            blend_time: 0.2,
            pose_source: machine.add_node(PoseNode::make_play_animation(animation)),
        })
        .collect::<Vec<_>>();

    let walk_node = machine.add_node(PoseNode::make_blend_animations_by_index(
        index_parameter,
        poses,
    ));

    (
        machine.add_state(State::new("Attack", walk_node)),
        animations,
    )
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

    const ATTACK_INDEX: &'static str = "AttackIndex";

    pub async fn new(
        resource_manager: ResourceManager,
        definition: &BotDefinition,
        model: Handle<Node>,
        scene: &mut Scene,
        hips: Handle<Node>,
    ) -> Self {
        let mut resources = vec![
            &definition.idle_animation,
            &definition.walk_animation,
            &definition.scream_animation,
            &definition.dying_animation,
        ];
        resources.extend(definition.attack_animations.iter().map(|a| &a.path));

        let resources = ModelMap::new(resources, resource_manager.clone()).await;

        let aim_animation_resource =
            if definition.can_use_weapons && !definition.aim_animation.is_empty() {
                resource_manager
                    .request_model(&definition.aim_animation)
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
            resources[&definition.idle_animation].clone(),
            "Idle",
            &mut machine,
            scene,
            model,
        );

        let (walk_animation, walk_state) = create_play_animation_state(
            resources[&definition.walk_animation].clone(),
            "Walk",
            &mut machine,
            scene,
            model,
        );

        let (scream_animation, scream_state) = create_play_animation_state(
            resources[&definition.scream_animation].clone(),
            "Scream",
            &mut machine,
            scene,
            model,
        );

        let (attack_state, attack_animations) = make_attack_state(
            &mut machine,
            scene,
            model,
            Self::ATTACK_INDEX.to_owned(),
            definition
                .attack_animations
                .iter()
                .map(|a| (resources[&a.path].clone(), a.timestamp))
                .collect(),
            hips,
        );

        let (dying_animation, dying_state) = create_play_animation_state(
            resources[&definition.dying_animation].clone(),
            "Dying",
            &mut machine,
            scene,
            model,
        );

        scene
            .animations
            .get_mut(dying_animation)
            .set_loop(false)
            .set_enabled(false);

        for leg_name in &[&definition.left_leg_name, &definition.right_leg_name] {
            let leg_node = scene.graph.find_by_name(model, leg_name);

            for &animation in &[
                idle_animation,
                walk_animation,
                aim_animation,
                scream_animation,
                dying_animation,
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

            // HACK. Move into upper loop.
            for &attack_animation in attack_animations.iter() {
                scene
                    .animations
                    .get_mut(attack_animation)
                    .set_tracks_enabled_from(leg_node, false, &scene.graph)
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
            attack_animations,
            aim_state,
            dying_animation,
        }
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        clean_machine(&self.machine, scene)
    }

    pub fn apply(&mut self, scene: &mut Scene, time: GameTime, input: UpperBodyMachineInput) {
        let attack_animation_ended = scene.animations
            [self.attack_animations[input.attack_animation_index as usize]]
            .has_ended();

        self.machine
            .set_parameter(
                Self::ATTACK_TO_IDLE,
                Parameter::Rule(!input.walk && attack_animation_ended),
            )
            .set_parameter(
                Self::ATTACK_TO_WALK,
                Parameter::Rule(input.walk && attack_animation_ended),
            )
            .set_parameter(
                Self::ATTACK_INDEX,
                Parameter::Index(input.attack_animation_index),
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
        self.attack_animations.visit("AttackAnimations", visitor)?;
        self.dying_animation.visit("DyingAnimation", visitor)?;
        self.aim_state.visit("AimState", visitor)?;

        visitor.leave_region()
    }
}
