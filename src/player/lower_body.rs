use crate::{
    level::footstep_ray_check,
    message::Message,
    player::{make_hit_reaction_state, upper_body::CombatWeaponKind, HitReactionStateDefinition},
    utils::create_play_animation_state,
};
use rg3d::{
    animation::{
        machine::{
            blend_nodes::BlendPose, Machine, Parameter, PoseNode, PoseWeight, State, Transition,
        },
        Animation, AnimationSignal,
    },
    core::{
        algebra::Vector3,
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::{resource_manager::ResourceManager, ColliderHandle},
    resource::model::Model,
    scene::{node::Node, Scene},
};
use std::sync::mpsc::Sender;

struct WalkStateDefinition {
    state: Handle<State>,
    walk_animation: Handle<Animation>,
    run_animation: Handle<Animation>,
}

fn make_walk_state(
    machine: &mut Machine,
    scene: &mut Scene,
    model: Handle<Node>,
    walk_animation_resource: Model,
    run_animation_resource: Model,
    walk_factor: String,
    run_factor: String,
) -> WalkStateDefinition {
    let walk_animation = *walk_animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    let walk_animation_node = machine.add_node(PoseNode::make_play_animation(walk_animation));

    let run_animation = *run_animation_resource
        .retarget_animations(model, scene)
        .get(0)
        .unwrap();
    let run_animation_node = machine.add_node(PoseNode::make_play_animation(run_animation));

    let walk_node = machine.add_node(PoseNode::make_blend_animations(vec![
        BlendPose::new(PoseWeight::Parameter(walk_factor), walk_animation_node),
        BlendPose::new(PoseWeight::Parameter(run_factor), run_animation_node),
    ]));

    WalkStateDefinition {
        state: machine.add_state(State::new("Walk", walk_node)),
        walk_animation,
        run_animation,
    }
}

#[derive(Default, Visit)]
pub struct LowerBodyMachine {
    pub machine: Machine,
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
    pub model: Handle<Node>,
}

pub struct LowerBodyMachineInput {
    pub is_walking: bool,
    pub is_jumping: bool,
    pub run_factor: f32,
    pub has_ground_contact: bool,
    pub is_dead: bool,
    pub should_be_stunned: bool,
    pub weapon_kind: CombatWeaponKind,
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

    const LAND_TO_DYING: &'static str = "LandToDying";
    const FALL_TO_DYING: &'static str = "FallToDying";
    const IDLE_TO_DYING: &'static str = "IdleToDying";
    const WALK_TO_DYING: &'static str = "WalkToDying";
    const JUMP_TO_DYING: &'static str = "JumpToDying";

    const IDLE_TO_HIT_REACTION: &'static str = "IdleToHitReaction";
    const WALK_TO_HIT_REACTION: &'static str = "WalkToHitReaction";
    const HIT_REACTION_TO_IDLE: &'static str = "HitReactionToIdle";
    const HIT_REACTION_TO_WALK: &'static str = "HitReactionToWalk";
    const HIT_REACTION_TO_DYING: &'static str = "HitReactionToDying";

    pub const JUMP_SIGNAL: u64 = 1;
    pub const LANDING_SIGNAL: u64 = 2;
    pub const FOOTSTEP_SIGNAL: u64 = 3;

    const RUN_FACTOR: &'static str = "RunFactor";
    const WALK_FACTOR: &'static str = "WalkFactor";
    const HIT_REACTION_WEAPON_KIND: &'static str = "HitReactionWeaponKind";

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
            dying_animation_resource,
            hit_reaction_rifle_animation_resource,
            hit_reaction_pistol_animation_resource,
        ) = rg3d::core::futures::join!(
            resource_manager.request_model(
                "data/animations/agent_walking_lower_body.fbx",
                Default::default()
            ),
            resource_manager.request_model("data/animations/agent_idle.fbx", Default::default()),
            resource_manager.request_model("data/animations/agent_jump.fbx", Default::default()),
            resource_manager.request_model("data/animations/agent_falling.fbx", Default::default()),
            resource_manager.request_model("data/animations/agent_landing.fbx", Default::default()),
            resource_manager
                .request_model("data/animations/agent_run_rifle.fbx", Default::default()),
            resource_manager.request_model("data/animations/agent_dying.fbx", Default::default()),
            resource_manager.request_model(
                "data/animations/agent_hit_reaction_rifle.fbx",
                Default::default()
            ),
            resource_manager.request_model(
                "data/animations/agent_hit_reaction_pistol.fbx",
                Default::default()
            ),
        );

        let HitReactionStateDefinition {
            state: hit_reaction_state,
            hit_reaction_pistol_animation,
            hit_reaction_rifle_animation,
        } = make_hit_reaction_state(
            &mut machine,
            scene,
            model,
            Self::HIT_REACTION_WEAPON_KIND.to_owned(),
            hit_reaction_rifle_animation_resource.unwrap(),
            hit_reaction_pistol_animation_resource.unwrap(),
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

        let (dying_animation, dying_state) = create_play_animation_state(
            dying_animation_resource.unwrap(),
            "Dying",
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
            .set_enabled(false)
            .add_signal(AnimationSignal::new(Self::JUMP_SIGNAL, 0.15))
            .set_loop(false);

        scene
            .animations
            .get_mut(land_animation)
            .add_signal(AnimationSignal::new(Self::LANDING_SIGNAL, 0.1))
            .set_loop(false);

        scene
            .animations
            .get_mut(walk_animation)
            .set_speed(0.5)
            .add_signal(AnimationSignal::new(Self::FOOTSTEP_SIGNAL, 0.4))
            .add_signal(AnimationSignal::new(Self::FOOTSTEP_SIGNAL, 0.8));

        scene
            .animations
            .get_mut(run_animation)
            .add_signal(AnimationSignal::new(Self::FOOTSTEP_SIGNAL, 0.25))
            .add_signal(AnimationSignal::new(Self::FOOTSTEP_SIGNAL, 0.5));

        scene
            .animations
            .get_mut(land_animation)
            .add_signal(AnimationSignal::new(Self::FOOTSTEP_SIGNAL, 0.12));

        scene
            .animations
            .get_mut(dying_animation)
            .set_enabled(false)
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
            0.40,
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

        // Dying transitions.
        machine.add_transition(Transition::new(
            "Land->Dying",
            land_state,
            dying_state,
            0.20,
            Self::LAND_TO_DYING,
        ));
        machine.add_transition(Transition::new(
            "Fall->Dying",
            fall_state,
            dying_state,
            0.20,
            Self::FALL_TO_DYING,
        ));
        machine.add_transition(Transition::new(
            "Idle->Dying",
            idle_state,
            dying_state,
            0.20,
            Self::IDLE_TO_DYING,
        ));
        machine.add_transition(Transition::new(
            "Walk->Dying",
            walk_state,
            dying_state,
            0.20,
            Self::WALK_TO_DYING,
        ));
        machine.add_transition(Transition::new(
            "Jump->Dying",
            jump_state,
            dying_state,
            0.20,
            Self::JUMP_TO_DYING,
        ));

        machine.add_transition(Transition::new(
            "Idle->Hit",
            idle_state,
            hit_reaction_state,
            0.20,
            Self::IDLE_TO_HIT_REACTION,
        ));
        machine.add_transition(Transition::new(
            "Walk->Hit",
            walk_state,
            hit_reaction_state,
            0.20,
            Self::WALK_TO_HIT_REACTION,
        ));
        machine.add_transition(Transition::new(
            "HitReaction->Idle",
            hit_reaction_state,
            idle_state,
            0.20,
            Self::HIT_REACTION_TO_IDLE,
        ));
        machine.add_transition(Transition::new(
            "HitReaction->Walk",
            hit_reaction_state,
            walk_state,
            0.20,
            Self::HIT_REACTION_TO_WALK,
        ));
        machine.add_transition(Transition::new(
            "HitReaction->Dying",
            hit_reaction_state,
            dying_state,
            0.20,
            Self::HIT_REACTION_TO_DYING,
        ));

        machine.set_entry_state(idle_state);

        Self {
            machine,
            jump_animation,
            walk_animation,
            run_animation,
            land_animation,
            dying_animation,
            hit_reaction_pistol_animation,
            hit_reaction_rifle_animation,
            walk_state,
            jump_state,
            fall_state,
            land_state,
            walk_to_jump,
            idle_to_jump,
            model,
        }
    }

    pub fn apply(
        &mut self,
        scene: &mut Scene,
        dt: f32,
        input: LowerBodyMachineInput,
        sender: &Sender<Message>,
        has_ground_contact: bool,
        self_collider: ColliderHandle,
    ) {
        let (current_hit_reaction_animation, index) = match input.weapon_kind {
            CombatWeaponKind::Rifle => (self.hit_reaction_rifle_animation, 0),
            CombatWeaponKind::Pistol => (self.hit_reaction_pistol_animation, 1),
        };
        let recovered = !input.should_be_stunned
            && scene.animations[current_hit_reaction_animation].has_ended();

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
            .set_parameter(Self::LAND_TO_DYING, Parameter::Rule(input.is_dead))
            .set_parameter(Self::IDLE_TO_DYING, Parameter::Rule(input.is_dead))
            .set_parameter(Self::FALL_TO_DYING, Parameter::Rule(input.is_dead))
            .set_parameter(Self::WALK_TO_DYING, Parameter::Rule(input.is_dead))
            .set_parameter(Self::JUMP_TO_DYING, Parameter::Rule(input.is_dead))
            .set_parameter(Self::HIT_REACTION_WEAPON_KIND, Parameter::Index(index))
            .set_parameter(
                Self::IDLE_TO_HIT_REACTION,
                Parameter::Rule(input.should_be_stunned),
            )
            .set_parameter(
                Self::WALK_TO_HIT_REACTION,
                Parameter::Rule(input.should_be_stunned),
            )
            .set_parameter(Self::HIT_REACTION_TO_IDLE, Parameter::Rule(recovered))
            .set_parameter(Self::HIT_REACTION_TO_WALK, Parameter::Rule(recovered))
            .set_parameter(Self::HIT_REACTION_TO_DYING, Parameter::Rule(input.is_dead))
            .set_parameter(Self::WALK_FACTOR, Parameter::Weight(1.0 - input.run_factor))
            .set_parameter(Self::RUN_FACTOR, Parameter::Weight(input.run_factor))
            .evaluate_pose(&scene.animations, dt)
            .apply(&mut scene.graph);

        let begin = scene.graph[self.model].global_position() + Vector3::new(0.0, 0.5, 0.0);

        while let Some((walking, evt)) = scene
            .animations
            .get_mut(self.walk_animation)
            .pop_event()
            .map(|e| (true, e))
            .or_else(|| {
                scene
                    .animations
                    .get_mut(self.run_animation)
                    .pop_event()
                    .map(|e| (false, e))
            })
        {
            if input.is_walking
                && has_ground_contact
                && evt.signal_id == Self::FOOTSTEP_SIGNAL
                && input.run_factor < 0.5
                && walking
                || input.run_factor >= 0.5 && !walking
            {
                footstep_ray_check(begin, scene, self_collider, sender.clone());
            }
        }

        while let Some(evt) = scene.animations.get_mut(self.land_animation).pop_event() {
            if evt.signal_id == Self::FOOTSTEP_SIGNAL {
                footstep_ray_check(begin, scene, self_collider, sender.clone());
            }
        }
    }

    pub fn is_stunned(&self, scene: &Scene) -> bool {
        let hr_animation = &scene.animations[self.hit_reaction_rifle_animation];
        !hr_animation.has_ended() && hr_animation.is_enabled()
    }

    pub fn hit_reaction_animations(&self) -> [Handle<Animation>; 2] {
        [
            self.hit_reaction_rifle_animation,
            self.hit_reaction_pistol_animation,
        ]
    }
}
