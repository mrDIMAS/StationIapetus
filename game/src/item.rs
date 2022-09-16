use crate::{current_level_mut, weapon::definition::WeaponKind};
use fyrox::{
    core::{
        color::Color,
        inspect::prelude::*,
        pool::Handle,
        reflect::Reflect,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    impl_component_provider,
    lazy_static::lazy_static,
    scene::{
        base::BaseBuilder, graph::map::NodeHandleMap, graph::Graph, node::Node,
        node::TypeUuidProvider, sprite::SpriteBuilder,
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};
use serde::Deserialize;
use std::{collections::HashMap, fs::File};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(
    Copy,
    Clone,
    PartialEq,
    Eq,
    Debug,
    Deserialize,
    Hash,
    Visit,
    Reflect,
    Inspect,
    AsRefStr,
    EnumString,
    EnumVariantNames,
)]
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

#[derive(Visit, Reflect, Inspect, Debug, Clone)]
pub struct Item {
    kind: ItemKind,
    model: Handle<Node>,
    pub stack_size: u32,

    #[inspect(skip)]
    #[reflect(hidden)]
    spark: Handle<Node>,

    #[inspect(skip)]
    #[reflect(hidden)]
    spark_size_change_dir: f32,

    #[inspect(skip)]
    #[reflect(hidden)]
    #[visit(skip)]
    pub definition: &'static ItemDefinition,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            kind: ItemKind::Medkit,
            model: Default::default(),
            spark: Default::default(),
            spark_size_change_dir: 1.0,
            stack_size: 1,
            definition: Self::get_definition(ItemKind::Medkit),
        }
    }
}

impl_component_provider!(Item);

impl TypeUuidProvider for Item {
    fn type_uuid() -> Uuid {
        uuid!("b915fa9e-6fd0-420d-8879-33cf76adfb5e")
    }
}

impl ScriptTrait for Item {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        self.definition = Self::get_definition(self.kind);

        // Create spark from code, since it is the same across all items.
        self.spark = SpriteBuilder::new(BaseBuilder::new().with_depth_offset(0.0025))
            .with_size(0.04)
            .with_color(Color::from_rgba(255, 255, 255, 160))
            .with_texture(
                ctx.resource_manager
                    .request_texture("data/particles/star_09.png"),
            )
            .build(&mut ctx.scene.graph);

        ctx.scene.graph.link_nodes(self.spark, ctx.handle);

        current_level_mut(ctx.plugins)
            .unwrap()
            .items
            .container
            .push(ctx.handle);
    }

    fn on_deinit(&mut self, context: &mut ScriptDeinitContext) {
        if let Some(level) = current_level_mut(context.plugins) {
            if let Some(index) = level
                .items
                .container
                .iter()
                .position(|i| *i == context.node_handle)
            {
                level.items.container.remove(index);
            }
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        let spark = ctx.scene.graph[self.spark].as_sprite_mut();
        let new_size = spark.size() + 0.02 * self.spark_size_change_dir * ctx.dt;
        spark.set_size(new_size);
        let new_rotation = spark.rotation() + 20.0f32.to_radians() * ctx.dt;
        spark.set_rotation(new_rotation);
        if spark.size() >= 0.04 {
            self.spark_size_change_dir = -1.0;
        } else if spark.size() < 0.03 {
            self.spark_size_change_dir = 1.0;
        }
    }

    fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping.map(&mut self.model);
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

#[derive(Deserialize, Debug)]
pub struct ItemDefinition {
    pub model: String,
    pub description: String,
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

    pub fn get_kind(&self) -> ItemKind {
        self.kind
    }
}

#[derive(Visit)]
pub struct ItemContainer {
    container: Vec<Handle<Node>>,
}

impl Default for ItemContainer {
    fn default() -> Self {
        Self::new()
    }
}

impl ItemContainer {
    pub fn new() -> Self {
        Self {
            container: Default::default(),
        }
    }

    pub fn contains(&self, item: Handle<Node>) -> bool {
        self.container.contains(&item)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Handle<Node>> {
        self.container.iter()
    }
}

pub fn item_ref(handle: Handle<Node>, graph: &Graph) -> &Item {
    graph[handle].try_get_script::<Item>().unwrap()
}

pub fn item_mut(handle: Handle<Node>, graph: &mut Graph) -> &mut Item {
    graph[handle].try_get_script_mut::<Item>().unwrap()
}
