use crate::{
    bot::BotDefinition,
    utils,
    utils::{create_play_animation_state, model_map::ModelMap},
};
use fyrox::{
    animation::{
        machine::{
            node::blend::IndexedBlendInput, LayerMask, Machine, MachineLayer, Parameter, PoseNode,
            State, Transition,
        },
        value::ValueBinding,
        Animation, AnimationSignal,
    },
    core::{
        pool::Handle,
        uuid::{uuid, Uuid},
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    resource::model::Model,
    scene::{node::Node, Scene},
};

#[derive(Default, Visit, Clone, Debug)]
pub struct UpperBodyMachine {
    pub machine: Machine,
    pub attack_animations: Vec<Handle<Animation>>,
    pub aim_state: Handle<State>,
    #[visit(optional)]
    pub attack_state: Handle<State>,
    pub dying_animation: Handle<Animation>,
    pub scream_animation: Handle<Animation>,
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

pub struct AttackAnimation {
    resource: Model,
    stick_timestamp: f32,
    timestamp: f32,
    speed: f32,
}

pub fn make_attack_state(
    layer: &mut MachineLayer,
    scene: &mut Scene,
    model: Handle<Node>,
    index_parameter: String,
    attack_animation_resources: Vec<AttackAnimation>,
    hips: Handle<Node>,
    animations_player: Handle<Node>,
) -> (Handle<State>, Vec<Handle<Animation>>) {
    let animations = attack_animation_resources
        .into_iter()
        .map(|desc| {
            let animation = *desc
                .resource
                .retarget_animations_to_player(model, animations_player, &mut scene.graph)
                .get(0)
                .unwrap();
            utils::fetch_animation_container_mut(&mut scene.graph, animations_player)[animation]
                .set_enabled(false)
                .set_loop(false)
                .add_signal(AnimationSignal {
                    id: UpperBodyMachine::STICK_SIGNAL,
                    name: "Stick".to_string(),
                    time: desc.stick_timestamp,
                    enabled: true,
                })
                .add_signal(AnimationSignal {
                    id: UpperBodyMachine::HIT_SIGNAL,
                    name: "Hit".to_string(),
                    time: desc.timestamp,
                    enabled: true,
                })
                .set_speed(desc.speed)
                .tracks_of_mut(hips)
                .filter(|t| t.binding() == &ValueBinding::Rotation)
                .for_each(|t| t.set_enabled(false));
            animation
        })
        .collect::<Vec<_>>();

    let poses = animations
        .iter()
        .map(|&animation| IndexedBlendInput {
            blend_time: 0.2,
            pose_source: layer.add_node(PoseNode::make_play_animation(animation)),
        })
        .collect::<Vec<_>>();

    let walk_node = layer.add_node(PoseNode::make_blend_animations_by_index(
        index_parameter,
        poses,
    ));

    (layer.add_state(State::new("Attack", walk_node)), animations)
}

impl UpperBodyMachine {
    pub const STICK_SIGNAL: Uuid = uuid!("8713a7e0-52cc-4745-8aa5-20f423f6fb92");
    pub const HIT_SIGNAL: Uuid = uuid!("17e3a824-c7b3-4aac-9ead-9c611737e213");

    const ATTACK_TO_IDLE: &'static str = "AttackToIdle";
    const ATTACK_TO_WALK: &'static str = "AttackToWalk";
    const IDLE_TO_ATTACK: &'static str = "IdleToAttack";
    const WALK_TO_ATTACK: &'static str = "WalkToAttack";
    const IDLE_TO_WALK: &'static str = "IdleToWalk";
    const WALK_TO_IDLE: &'static str = "WalkToIdle";
    const IDLE_TO_SCREAM: &'static str = "IdleToScream";
    const WALK_TO_SCREAM: &'static str = "WalkToScream";
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
        animations_player: Handle<Node>,
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

        let root_layer = machine.layers_mut().first_mut().unwrap();

        let mut layer_mask = LayerMask::default();
        for leg_name in &[&definition.left_leg_name, &definition.right_leg_name] {
            let leg_node = scene.graph.find_by_name(model, leg_name).unwrap().0;
            layer_mask.merge(LayerMask::from_hierarchy(&scene.graph, leg_node));
        }
        root_layer.set_mask(layer_mask);

        let (_, aim_state) = if let Some(aim_animation_resource) = aim_animation_resource.clone() {
            create_play_animation_state(
                aim_animation_resource,
                "Aim",
                root_layer,
                scene,
                model,
                animations_player,
            )
        } else {
            (Handle::NONE, Handle::NONE)
        };

        let (_, idle_state) = create_play_animation_state(
            resources[&definition.idle_animation].clone(),
            "Idle",
            root_layer,
            scene,
            model,
            animations_player,
        );

        let (_, walk_state) = create_play_animation_state(
            resources[&definition.walk_animation].clone(),
            "Walk",
            root_layer,
            scene,
            model,
            animations_player,
        );

        let (scream_animation, scream_state) = create_play_animation_state(
            resources[&definition.scream_animation].clone(),
            "Scream",
            root_layer,
            scene,
            model,
            animations_player,
        );

        let (attack_state, attack_animations) = make_attack_state(
            root_layer,
            scene,
            model,
            Self::ATTACK_INDEX.to_owned(),
            definition
                .attack_animations
                .iter()
                .map(|a| AttackAnimation {
                    resource: resources[&a.path].clone(),
                    stick_timestamp: a.stick_timestamp,
                    timestamp: a.timestamp,
                    speed: a.speed,
                })
                .collect(),
            hips,
            animations_player,
        );

        let (dying_animation, dying_state) = create_play_animation_state(
            resources[&definition.dying_animation].clone(),
            "Dying",
            root_layer,
            scene,
            model,
            animations_player,
        );

        let animations_container_mut =
            utils::fetch_animation_container_mut(&mut scene.graph, animations_player);

        animations_container_mut
            .get_mut(scream_animation)
            .set_loop(false)
            .set_enabled(false)
            .set_speed(1.0);

        animations_container_mut
            .get_mut(dying_animation)
            .set_loop(false)
            .set_enabled(false);

        root_layer.add_transition(Transition::new(
            "Attack->Idle",
            attack_state,
            idle_state,
            0.2,
            Self::ATTACK_TO_IDLE,
        ));
        root_layer.add_transition(Transition::new(
            "Attack->Walk",
            attack_state,
            walk_state,
            0.2,
            Self::ATTACK_TO_WALK,
        ));
        root_layer.add_transition(Transition::new(
            "Idle->Attack",
            idle_state,
            attack_state,
            0.2,
            Self::IDLE_TO_ATTACK,
        ));
        root_layer.add_transition(Transition::new(
            "Walk->Attack",
            walk_state,
            attack_state,
            0.2,
            Self::WALK_TO_ATTACK,
        ));
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
        if aim_animation_resource.is_some() {
            root_layer.add_transition(Transition::new(
                "Idle->Aim",
                idle_state,
                aim_state,
                0.2,
                Self::IDLE_TO_AIM,
            ));
            root_layer.add_transition(Transition::new(
                "Aim->Idle",
                aim_state,
                idle_state,
                0.2,
                Self::AIM_TO_IDLE,
            ));
            root_layer.add_transition(Transition::new(
                "Aim->Walk",
                aim_state,
                walk_state,
                0.2,
                Self::AIM_TO_WALK,
            ));
        }
        root_layer.add_transition(Transition::new(
            "Attack->Dying",
            attack_state,
            dying_state,
            0.2,
            Self::ATTACK_TO_DYING,
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
            machine,
            attack_animations,
            aim_state,
            dying_animation,
            scream_animation,
            attack_state,
        }
    }

    pub fn apply(
        &mut self,
        scene: &mut Scene,
        dt: f32,
        input: UpperBodyMachineInput,
        animations_player: Handle<Node>,
    ) {
        let animations_container_ref =
            utils::fetch_animation_container_ref(&scene.graph, animations_player);

        let attack_animation_ended = animations_container_ref
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
            .set_parameter(Self::WALK_TO_SCREAM, Parameter::Rule(input.scream))
            .set_parameter(Self::SCREAM_TO_WALK, Parameter::Rule(!input.scream))
            .set_parameter(Self::SCREAM_TO_IDLE, Parameter::Rule(!input.scream))
            .set_parameter(Self::IDLE_TO_AIM, Parameter::Rule(input.aim))
            .set_parameter(Self::AIM_TO_IDLE, Parameter::Rule(!input.aim))
            .set_parameter(Self::AIM_TO_WALK, Parameter::Rule(input.walk && !input.aim))
            .set_parameter(Self::ATTACK_TO_DYING, Parameter::Rule(input.dead))
            .set_parameter(Self::WALK_TO_DYING, Parameter::Rule(input.dead))
            .set_parameter(Self::IDLE_TO_DYING, Parameter::Rule(input.dead))
            .evaluate_pose(animations_container_ref, dt)
            .apply(&mut scene.graph);
    }

    /// Returns true if bot started to perform a swing to hit a target. This flag is used to
    /// modify speed of bot to speed it up if it too far away from the target.
    pub fn should_stick_to_target(&self, scene: &Scene, animations_player: Handle<Node>) -> bool {
        let animations_container_ref =
            utils::fetch_animation_container_ref(&scene.graph, animations_player);

        for &handle in self.attack_animations.iter() {
            let animation = &animations_container_ref[handle];
            if !animation.has_ended() && animation.is_enabled() {
                for signal in animation.signals() {
                    if signal.id == Self::HIT_SIGNAL && animation.time_position() > signal.time {
                        return false;
                    }
                    if signal.id == Self::STICK_SIGNAL && animation.time_position() > signal.time {
                        return true;
                    }
                }
            }
        }
        false
    }
}
