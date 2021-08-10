use crate::{
    actor::{Actor, TargetDescriptor},
    bot::{
        behavior::{
            aim::AimOnTarget,
            death::{IsDead, StayDead},
            find::FindTarget,
            melee::{CanMeleeAttack, DoMeleeAttack},
            movement::MoveToTarget,
            shoot::{CanShootTarget, ShootTarget},
        },
        lower_body::LowerBodyMachine,
        upper_body::UpperBodyMachine,
        BotDefinition, BotKind, Target,
    },
    character::Character,
    message::Message,
    utils::BodyImpactHandler,
    weapon::WeaponContainer,
    GameTime,
};
use rg3d::core::math::SmoothAngle;
use rg3d::{
    core::{pool::Handle, visitor::prelude::*},
    scene::{node::Node, Scene},
    utils::{
        behavior::{
            composite::{CompositeNode, CompositeNodeKind},
            leaf::LeafNode,
            Behavior, BehaviorTree, Status,
        },
        navmesh::{Navmesh, NavmeshAgent},
    },
};
use std::sync::mpsc::Sender;

pub mod aim;
pub mod death;
pub mod find;
pub mod melee;
pub mod movement;
pub mod shoot;

#[derive(Debug, PartialEq, Visit)]
pub enum Action {
    Unknown,
    IsDead(IsDead),
    StayDead(StayDead),
    FindTarget(FindTarget),
    MoveToTarget(MoveToTarget),
    CanMeleeAttack(CanMeleeAttack),
    AimOnTarget(AimOnTarget),
    DoMeleeAttack(DoMeleeAttack),
    CanShootTarget(CanShootTarget),
    ShootTarget(ShootTarget),
}

impl Default for Action {
    fn default() -> Self {
        Action::Unknown
    }
}

impl<'a> Behavior<'a> for Action {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        match self {
            Action::Unknown => unreachable!(),
            Action::FindTarget(v) => v.tick(context),
            Action::MoveToTarget(v) => v.tick(context),
            Action::DoMeleeAttack(v) => v.tick(context),
            Action::ShootTarget(v) => v.tick(context),
            Action::CanMeleeAttack(v) => v.tick(context),
            Action::IsDead(v) => v.tick(context),
            Action::StayDead(v) => v.tick(context),
            Action::AimOnTarget(v) => v.tick(context),
            Action::CanShootTarget(v) => v.tick(context),
        }
    }
}

pub struct BehaviorContext<'a> {
    pub scene: &'a mut Scene,
    pub bot_handle: Handle<Actor>,
    pub targets: &'a [TargetDescriptor],
    pub weapons: &'a WeaponContainer,
    pub sender: &'a Sender<Message>,
    pub time: GameTime,
    pub navmesh: Handle<Navmesh>,
    pub upper_body_machine: &'a UpperBodyMachine,
    pub lower_body_machine: &'a LowerBodyMachine,
    pub target: &'a mut Option<Target>,
    pub definition: &'static BotDefinition,
    pub character: &'a mut Character,
    pub kind: BotKind,
    pub agent: &'a mut NavmeshAgent,
    pub impact_handler: &'a BodyImpactHandler,
    pub model: Handle<Node>,
    pub restoration_time: f32,
    pub v_recoil: &'a mut SmoothAngle,
    pub h_recoil: &'a mut SmoothAngle,

    // Output
    pub attack_animation_index: usize,
    pub movement_speed_factor: f32,
    pub is_moving: bool,
    pub is_attacking: bool,
    pub is_aiming_weapon: bool,
}

#[derive(Default, Visit)]
pub struct BotBehavior {
    pub tree: BehaviorTree<Action>,
}

impl BotBehavior {
    pub fn new(spine: Handle<Node>, definition: &BotDefinition) -> Self {
        let mut tree = BehaviorTree::new();

        let entry = CompositeNode::new(
            CompositeNodeKind::Selector,
            vec![
                CompositeNode::new(
                    CompositeNodeKind::Sequence,
                    vec![
                        LeafNode::new(Action::IsDead(IsDead)).add(&mut tree),
                        LeafNode::new(Action::StayDead(StayDead)).add(&mut tree),
                    ],
                )
                .add(&mut tree),
                CompositeNode::new(
                    CompositeNodeKind::Sequence,
                    vec![
                        LeafNode::new(Action::FindTarget(FindTarget::default())).add(&mut tree),
                        CompositeNode::new(
                            CompositeNodeKind::Sequence,
                            vec![
                                LeafNode::new(Action::AimOnTarget(AimOnTarget::new(spine)))
                                    .add(&mut tree),
                                CompositeNode::new(
                                    CompositeNodeKind::Selector,
                                    vec![
                                        CompositeNode::new(
                                            CompositeNodeKind::Sequence,
                                            vec![
                                                LeafNode::new(Action::CanShootTarget(
                                                    CanShootTarget,
                                                ))
                                                .add(&mut tree),
                                                LeafNode::new(Action::MoveToTarget(MoveToTarget {
                                                    min_distance: 4.0,
                                                }))
                                                .add(&mut tree),
                                                LeafNode::new(Action::ShootTarget(ShootTarget))
                                                    .add(&mut tree),
                                            ],
                                        )
                                        .add(&mut tree),
                                        CompositeNode::new(
                                            CompositeNodeKind::Sequence,
                                            vec![
                                                LeafNode::new(Action::MoveToTarget(MoveToTarget {
                                                    min_distance: definition.close_combat_distance,
                                                }))
                                                .add(&mut tree),
                                                LeafNode::new(Action::CanMeleeAttack(
                                                    CanMeleeAttack,
                                                ))
                                                .add(&mut tree),
                                                LeafNode::new(Action::DoMeleeAttack(
                                                    DoMeleeAttack::default(),
                                                ))
                                                .add(&mut tree),
                                            ],
                                        )
                                        .add(&mut tree),
                                    ],
                                )
                                .add(&mut tree),
                            ],
                        )
                        .add(&mut tree),
                    ],
                )
                .add(&mut tree),
            ],
        )
        .add(&mut tree);

        tree.set_entry_node(entry);

        Self { tree }
    }
}
