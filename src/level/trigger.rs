use crate::{actor::ActorContainer, message::Message, MessageSender};
use rg3d::{
    core::{
        pool::{Handle, Pool},
        visitor::{Visit, VisitResult, Visitor},
    },
    scene::{node::Node, Scene},
};

#[derive(Visit)]
pub enum TriggerKind {
    NextLevel,
    EndGame,
}

impl Default for TriggerKind {
    fn default() -> Self {
        Self::NextLevel
    }
}

#[derive(Default, Visit)]
pub struct Trigger {
    node: Handle<Node>,
    kind: TriggerKind,
}

impl Trigger {
    pub fn new(node: Handle<Node>, kind: TriggerKind) -> Self {
        Self { node, kind }
    }
}

#[derive(Default, Visit)]
pub struct TriggerContainer {
    pool: Pool<Trigger>,
}

impl TriggerContainer {
    pub fn add(&mut self, trigger: Trigger) {
        let _ = self.pool.spawn(trigger);
    }

    pub fn update(&mut self, scene: &Scene, actors: &ActorContainer, sender: &MessageSender) {
        for trigger in self.pool.iter() {
            let position = scene.graph[trigger.node].global_position();

            for actor in actors.iter() {
                let actor_position = actor.position(&scene.graph);

                if actor_position.metric_distance(&position) < 1.0 {
                    match trigger.kind {
                        TriggerKind::NextLevel => sender.send(Message::LoadNextLevel),
                        TriggerKind::EndGame => sender.send(Message::EndGame),
                    }
                }
            }
        }
    }
}
