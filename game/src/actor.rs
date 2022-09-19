use crate::bot::BotKind;
use fyrox::core::{algebra::Vector3, pool::Handle};
use fyrox::scene::node::Node;

pub enum TargetKind {
    Player,
    Bot(BotKind),
}

// Helper struct that used to hold information about possible target for bots
// it contains all needed information to select suitable target. This is needed
// because of borrowing rules that does not allows to have a mutable reference
// to array element and iterate over array using immutable borrow.
pub struct TargetDescriptor {
    pub handle: Handle<Node>,
    pub health: f32,
    pub position: Vector3<f32>,
    pub kind: TargetKind,
}
