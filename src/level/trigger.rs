use crate::{actor::ActorContainer, message::Message};
use rg3d::{
    core::{
        pool::{Handle, Pool},
        visitor::{Visit, VisitResult, Visitor},
    },
    scene::{node::Node, Scene},
};
use std::sync::mpsc::Sender;

pub enum TriggerKind {
    NextLevel,
    EndGame,
}

impl Default for TriggerKind {
    fn default() -> Self {
        Self::NextLevel
    }
}

impl Visit for TriggerKind {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        let mut id = self.id();
        id.visit(name, visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(id)?;
        }
        Ok(())
    }
}

impl TriggerKind {
    fn id(&self) -> u32 {
        match self {
            TriggerKind::NextLevel => 0,
            TriggerKind::EndGame => 1,
        }
    }

    fn from_id(id: u32) -> Result<Self, String> {
        match id {
            0 => Ok(Self::NextLevel),
            1 => Ok(Self::EndGame),
            _ => Err(format!("Invalid trigger id {}!", id)),
        }
    }
}

#[derive(Default)]
pub struct Trigger {
    node: Handle<Node>,
    kind: TriggerKind,
}

impl Trigger {
    pub fn new(node: Handle<Node>, kind: TriggerKind) -> Self {
        Self { node, kind }
    }
}

impl Visit for Trigger {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.node.visit("Node", visitor)?;
        self.kind.visit("Kind", visitor)?;

        visitor.leave_region()
    }
}

#[derive(Default)]
pub struct TriggerContainer {
    pool: Pool<Trigger>,
}

impl Visit for TriggerContainer {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.pool.visit("Pool", visitor)?;

        visitor.leave_region()
    }
}

impl TriggerContainer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, trigger: Trigger) {
        let _ = self.pool.spawn(trigger);
    }

    pub fn update(&mut self, scene: &Scene, actors: &ActorContainer, sender: &Sender<Message>) {
        for trigger in self.pool.iter() {
            let position = scene.graph[trigger.node].global_position();

            for actor in actors.iter() {
                let actor_position = actor.position(&scene.graph);

                if actor_position.metric_distance(&position) < 1.0 {
                    match trigger.kind {
                        TriggerKind::NextLevel => sender.send(Message::LoadNextLevel).unwrap(),
                        TriggerKind::EndGame => sender.send(Message::EndGame).unwrap(),
                    }
                }
            }
        }
    }
}
