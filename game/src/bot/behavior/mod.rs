use crate::{
    bot::{
        behavior::{
            aim::{AimOnTarget, AimTarget},
            death::{IsDead, StayDead},
            find::FindTarget,
            melee::{CanMeleeAttack, DoMeleeAttack},
            movement::MoveToTarget,
            range::IsTargetCloseBy,
            shoot::{CanShootTarget, ShootTarget},
            threat::{NeedsThreatenTarget, ThreatenTarget},
        },
        state_machine::StateMachine,
        BotHostility, Target,
    },
    character::Character,
    sound::SoundManager,
    utils::{BodyImpactHandler, ResourceProxy},
    MessageSender,
};
use fyrox::{
    core::{math::SmoothAngle, pool::Handle, visitor::prelude::*},
    plugin::Plugin,
    scene::{node::Node, sound::SoundBufferResource, Scene},
    script::ScriptMessageSender,
    utils::{behavior::*, navmesh::NavmeshAgent},
};

pub mod aim;
pub mod death;
pub mod find;
pub mod melee;
pub mod movement;
pub mod range;
pub mod shoot;
pub mod threat;

#[derive(Debug, PartialEq, Visit, Clone, Default)]
pub enum Action {
    #[default]
    Unknown,
    IsDead(IsDead),
    StayDead(StayDead),
    FindTarget(FindTarget),
    ReachedTarget(IsTargetCloseBy),
    MoveToTarget(MoveToTarget),
    CanMeleeAttack(CanMeleeAttack),
    AimOnTarget(AimOnTarget),
    DoMeleeAttack(DoMeleeAttack),
    CanShootTarget(CanShootTarget),
    ShootTarget(ShootTarget),
    NeedsThreatenTarget(NeedsThreatenTarget),
    ThreatenTarget(ThreatenTarget),
}

impl<'a> Behavior<'a> for Action {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        match self {
            Action::Unknown => unreachable!(),
            Action::FindTarget(v) => v.tick(context),
            Action::ReachedTarget(v) => v.tick(context),
            Action::MoveToTarget(v) => v.tick(context),
            Action::DoMeleeAttack(v) => v.tick(context),
            Action::ShootTarget(v) => v.tick(context),
            Action::CanMeleeAttack(v) => v.tick(context),
            Action::IsDead(v) => v.tick(context),
            Action::StayDead(v) => v.tick(context),
            Action::AimOnTarget(v) => v.tick(context),
            Action::CanShootTarget(v) => v.tick(context),
            Action::NeedsThreatenTarget(v) => v.tick(context),
            Action::ThreatenTarget(v) => v.tick(context),
        }
    }
}

pub struct BehaviorContext<'a> {
    pub scene: &'a mut Scene,
    pub actors: &'a [Handle<Node>],
    pub bot_handle: Handle<Node>,
    pub sender: &'a MessageSender,
    pub dt: f32,
    pub elapsed_time: f32,
    pub state_machine: &'a StateMachine,
    pub target: &'a mut Option<Target>,
    pub character: &'a mut Character,
    pub agent: &'a mut NavmeshAgent,
    pub impact_handler: &'a BodyImpactHandler,
    pub model: Handle<Node>,
    pub restoration_time: f32,
    pub v_recoil: &'a mut SmoothAngle,
    pub h_recoil: &'a mut SmoothAngle,
    pub move_speed: f32,
    pub threaten_timeout: &'a mut f32,
    pub sound_manager: &'a SoundManager,
    pub animation_player: Handle<Node>,
    pub script_message_sender: &'a ScriptMessageSender,
    pub navmesh: Handle<Node>,
    pub hostility: BotHostility,
    pub h_aim_angle_hack: f32,
    pub v_aim_angle_hack: f32,
    pub attack_sounds: &'a [ResourceProxy<SoundBufferResource>],
    pub scream_sounds: &'a [ResourceProxy<SoundBufferResource>],
    pub yaw: &'a mut SmoothAngle,
    pub pitch: &'a mut SmoothAngle,
    pub plugins: &'a [Box<dyn Plugin>],

    // Output
    pub attack_animation_index: usize,
    pub movement_speed_factor: f32,
    pub is_moving: bool,
    pub is_attacking: bool,
    pub is_aiming_weapon: bool,
    pub is_screaming: bool,
}

#[derive(Default, Debug, Visit, Clone)]
pub struct BotBehavior {
    pub tree: BehaviorTree<Action>,
}

impl BotBehavior {
    pub fn new(spine: Handle<Node>, close_combat_distance: f32) -> Self {
        let mut tree = BehaviorTree::new();
        let bt = &mut tree;

        let dead_seq = sequence([IsDead::new_action(bt), StayDead::new_action(bt)], bt);

        let threaten_seq = sequence(
            [
                leaf(
                    Action::NeedsThreatenTarget(NeedsThreatenTarget::default()),
                    bt,
                ),
                leaf(AimOnTarget::new_action(spine, AimTarget::ActualTarget), bt),
                leaf(Action::ThreatenTarget(ThreatenTarget::default()), bt),
            ],
            bt,
        );

        let shooting_distance = 4.0;
        let shoot_seq = sequence(
            [
                leaf(Action::CanShootTarget(CanShootTarget), bt),
                selector(
                    [
                        sequence(
                            [
                                inverter(IsTargetCloseBy::make(shooting_distance, bt), bt),
                                leaf(
                                    AimOnTarget::new_action(spine, AimTarget::SteeringTarget),
                                    bt,
                                ),
                                leaf(
                                    Action::MoveToTarget(MoveToTarget {
                                        min_distance: shooting_distance,
                                    }),
                                    bt,
                                ),
                            ],
                            bt,
                        ),
                        leaf(AimOnTarget::new_action(spine, AimTarget::ActualTarget), bt),
                    ],
                    bt,
                ),
                leaf(Action::ShootTarget(ShootTarget), bt),
            ],
            bt,
        );

        let melee_seq = sequence(
            [
                selector(
                    [
                        sequence(
                            [
                                inverter(
                                    leaf(
                                        Action::ReachedTarget(IsTargetCloseBy {
                                            min_distance: close_combat_distance,
                                        }),
                                        bt,
                                    ),
                                    bt,
                                ),
                                leaf(
                                    AimOnTarget::new_action(spine, AimTarget::SteeringTarget),
                                    bt,
                                ),
                                leaf(
                                    Action::MoveToTarget(MoveToTarget {
                                        min_distance: close_combat_distance,
                                    }),
                                    bt,
                                ),
                            ],
                            bt,
                        ),
                        leaf(AimOnTarget::new_action(spine, AimTarget::ActualTarget), bt),
                    ],
                    bt,
                ),
                leaf(Action::CanMeleeAttack(CanMeleeAttack), bt),
                leaf(Action::DoMeleeAttack(DoMeleeAttack::default()), bt),
            ],
            bt,
        );

        let entry = selector(
            [
                dead_seq,
                sequence(
                    [
                        leaf(Action::FindTarget(FindTarget::default()), bt),
                        sequence([selector([threaten_seq, shoot_seq, melee_seq], bt)], bt),
                    ],
                    bt,
                ),
            ],
            bt,
        );

        tree.set_entry_node(entry);

        Self { tree }
    }
}
