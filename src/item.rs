use crate::weapon::definition::WeaponKind;
use rg3d::engine::resource_manager::MaterialSearchOptions;
use rg3d::{
    core::{
        algebra::Vector3,
        color::Color,
        pool::{Handle, Pool, PoolIterator, PoolPairIterator},
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    lazy_static::lazy_static,
    scene::{
        base::BaseBuilder, graph::Graph, node::Node, sprite::SpriteBuilder,
        transform::TransformBuilder, Scene,
    },
    sound::pool::PoolIteratorMut,
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File};

#[derive(Copy, Clone, PartialEq, Eq, Debug, Deserialize, Hash, Visit)]
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
    RailGun,

    // Keys
    MasterKey,
}

impl Default for ItemKind {
    fn default() -> Self {
        Self::Medkit
    }
}

impl ItemKind {
    pub fn associated_weapon(&self) -> Option<WeaponKind> {
        match self {
            ItemKind::PlasmaGun => Some(WeaponKind::PlasmaRifle),
            ItemKind::Ak47 => Some(WeaponKind::Ak47),
            ItemKind::M4 => Some(WeaponKind::M4),
            ItemKind::Glock => Some(WeaponKind::Glock),
            ItemKind::RailGun => Some(WeaponKind::RailGun),
            ItemKind::Medkit
            | ItemKind::Medpack
            | ItemKind::Ammo
            | ItemKind::Grenade
            | ItemKind::MasterKey => None,
        }
    }
}

pub struct Item {
    kind: ItemKind,
    pivot: Handle<Node>,
    model: Handle<Node>,
    spark: Handle<Node>,
    spark_size_change_dir: f32,
    pub stack_size: u32,
    pub definition: &'static ItemDefinition,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            kind: ItemKind::Medkit,
            pivot: Default::default(),
            model: Default::default(),
            spark: Default::default(),
            spark_size_change_dir: 1.0,
            stack_size: 1,
            definition: Self::get_definition(ItemKind::Medkit),
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
    pub preview: String,
}

#[derive(Deserialize, Default)]
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
        DEFINITIONS
            .map
            .get(&kind)
            .expect(&format!("No definition for {:?} weapon!", kind))
    }

    pub async fn new(
        kind: ItemKind,
        position: Vector3<f32>,
        scene: &mut Scene,
        resource_manager: ResourceManager,
    ) -> Self {
        let definition = Self::get_definition(kind);

        let spark;
        let model = resource_manager
            .request_model(&definition.model, MaterialSearchOptions::RecursiveUp)
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
            .with_children(&[model, {
                spark = SpriteBuilder::new(BaseBuilder::new().with_depth_offset(0.0025))
                    .with_size(0.04)
                    .with_color(Color::from_rgba(255, 255, 255, 160))
                    .with_texture(resource_manager.request_texture("data/particles/star_09.png"))
                    .build(&mut scene.graph);
                spark
            }])
            .build(&mut scene.graph);

        Self {
            pivot,
            kind,
            model,
            spark,
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

    fn update(&mut self, dt: f32, graph: &mut Graph) {
        let spark = graph[self.spark].as_sprite_mut();
        let new_size = spark.size() + 0.02 * self.spark_size_change_dir * dt;
        spark.set_size(new_size);
        let new_rotation = spark.rotation() + 20.0f32.to_radians() * dt;
        spark.set_rotation(new_rotation);
        if spark.size() >= 0.04 {
            self.spark_size_change_dir = -1.0;
        } else if spark.size() < 0.03 {
            self.spark_size_change_dir = 1.0;
        }
    }
}

impl Visit for Item {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.kind.visit("Kind", visitor)?;
        self.definition = Self::get_definition(self.kind);
        self.model.visit("Model", visitor)?;
        self.pivot.visit("Pivot", visitor)?;
        self.stack_size.visit("StackSize", visitor)?;
        self.spark.visit("Spark", visitor)?;
        self.spark_size_change_dir
            .visit("SparkSizeChangeDir", visitor)?;

        visitor.leave_region()
    }
}

#[derive(Visit)]
pub struct ItemContainer {
    pool: Pool<Item>,
}

impl Default for ItemContainer {
    fn default() -> Self {
        Self::new()
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

    pub fn remove(&mut self, item: Handle<Item>, graph: &mut Graph) {
        self.pool[item].cleanup(graph);
        self.pool.free(item);
    }

    pub fn update(&mut self, dt: f32, graph: &mut Graph) {
        for item in self.pool.iter_mut() {
            item.update(dt, graph);
        }
    }
}
