use crate::message::Message;
use rg3d::{
    core::{
        algebra::Vector3,
        pool::{Handle, Pool, PoolIterator, PoolPairIterator},
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    lazy_static::lazy_static,
    scene::{base::BaseBuilder, graph::Graph, node::Node, transform::TransformBuilder, Scene},
    sound::pool::PoolIteratorMut,
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File, sync::mpsc::Sender};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Hash)]
pub enum ItemKind {
    Medkit,
    Medpack,

    // Ammo
    Ammo,
    Grenade,

    // Weapons
    PlasmaGun,
    Ak47,
    M4,
    Glock,
}

impl Default for ItemKind {
    fn default() -> Self {
        Self::Medkit
    }
}

impl Visit for ItemKind {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        let mut kind = self.id();
        kind.visit(name, visitor)?;
        if visitor.is_reading() {
            *self = Self::from_id(kind)?;
        }
        Ok(())
    }
}

impl ItemKind {
    fn from_id(id: u32) -> Result<ItemKind, String> {
        match id {
            0 => Ok(ItemKind::Medkit),
            1 => Ok(ItemKind::Ammo),
            2 => Ok(ItemKind::Medpack),
            3 => Ok(ItemKind::Grenade),
            4 => Ok(ItemKind::PlasmaGun),
            5 => Ok(ItemKind::Ak47),
            6 => Ok(ItemKind::M4),
            7 => Ok(ItemKind::Glock),
            _ => Err(format!("Unknown item kind {}", id)),
        }
    }

    fn id(self) -> u32 {
        match self {
            ItemKind::Medkit => 0,
            ItemKind::Ammo => 1,
            ItemKind::Medpack => 2,
            ItemKind::Grenade => 3,
            ItemKind::PlasmaGun => 4,
            ItemKind::Ak47 => 5,
            ItemKind::M4 => 6,
            ItemKind::Glock => 7,
        }
    }
}

pub struct Item {
    kind: ItemKind,
    pivot: Handle<Node>,
    model: Handle<Node>,
    definition: &'static ItemDefinition,
    pub sender: Option<Sender<Message>>,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            kind: ItemKind::Medkit,
            pivot: Default::default(),
            model: Default::default(),
            definition: Self::get_definition(ItemKind::Medkit),
            sender: None,
        }
    }
}

#[derive(Deserialize)]
pub struct ItemDefinition {
    pub model: String,
    pub description: String,
    pub scale: f32,
    pub name: String,
    pub consumable: bool,
}

#[derive(Deserialize)]
pub struct ItemDefinitionContainer {
    map: HashMap<ItemKind, ItemDefinition>,
}

impl ItemDefinitionContainer {
    pub fn new() -> Self {
        let file = File::open("data/configs/items.ron").unwrap();
        ron::de::from_reader(file).unwrap()
    }
}

lazy_static! {
    static ref DEFINITIONS: ItemDefinitionContainer = ItemDefinitionContainer::new();
}

impl Item {
    pub fn get_definition(kind: ItemKind) -> &'static ItemDefinition {
        DEFINITIONS.map.get(&kind).unwrap()
    }

    pub async fn new(
        kind: ItemKind,
        position: Vector3<f32>,
        scene: &mut Scene,
        resource_manager: ResourceManager,
        sender: Sender<Message>,
    ) -> Self {
        let definition = Self::get_definition(kind);

        let model = resource_manager
            .request_model(&definition.model)
            .await
            .unwrap()
            .instantiate_geometry(scene);

        let pivot = BaseBuilder::new()
            .with_local_transform(
                TransformBuilder::new()
                    .with_local_position(position)
                    .with_local_scale(Vector3::new(
                        definition.scale,
                        definition.scale,
                        definition.scale,
                    ))
                    .build(),
            )
            .build(&mut scene.graph);

        scene.graph.link_nodes(model, pivot);

        Self {
            pivot,
            kind,
            model,
            sender: Some(sender),
            ..Default::default()
        }
    }

    pub fn get_pivot(&self) -> Handle<Node> {
        self.pivot
    }

    pub fn position(&self, graph: &Graph) -> Vector3<f32> {
        graph[self.pivot].global_position()
    }

    pub fn get_kind(&self) -> ItemKind {
        self.kind
    }

    fn cleanup(&self, graph: &mut Graph) {
        graph.remove_node(self.pivot)
    }
}

impl Visit for Item {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.kind.visit("Kind", visitor)?;
        self.definition = Self::get_definition(self.kind);
        self.model.visit("Model", visitor)?;
        self.pivot.visit("Pivot", visitor)?;

        visitor.leave_region()
    }
}

pub struct ItemContainer {
    pool: Pool<Item>,
}

impl Default for ItemContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl Visit for ItemContainer {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.pool.visit("Pool", visitor)?;

        visitor.leave_region()
    }
}

impl ItemContainer {
    pub fn new() -> Self {
        Self { pool: Pool::new() }
    }

    pub fn add(&mut self, item: Item) -> Handle<Item> {
        self.pool.spawn(item)
    }

    pub fn get_mut(&mut self, item: Handle<Item>) -> &mut Item {
        self.pool.borrow_mut(item)
    }

    pub fn contains(&self, item: Handle<Item>) -> bool {
        self.pool.is_valid_handle(item)
    }

    pub fn pair_iter(&self) -> PoolPairIterator<Item> {
        self.pool.pair_iter()
    }

    pub fn iter(&self) -> PoolIterator<Item> {
        self.pool.iter()
    }

    pub fn iter_mut(&mut self) -> PoolIteratorMut<Item> {
        self.pool.iter_mut()
    }
}
