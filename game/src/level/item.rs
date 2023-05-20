use crate::{block_on, Level};
use fyrox::core::log::Log;
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        color::Color,
        math::ray::Ray,
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
        TypeUuidProvider,
    },
    impl_component_provider,
    resource::{
        model::{ModelResource, ModelResourceExtension},
        texture::{Texture, TextureResource},
    },
    scene::{
        base::BaseBuilder, collider::ColliderShape, graph::physics::RayCastOptions, node::Node,
        sprite::SpriteBuilder, Scene,
    },
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};
use strum_macros::{AsRefStr, EnumString, EnumVariantNames};

#[derive(
    Default, Visit, Reflect, PartialEq, Debug, Clone, AsRefStr, EnumString, EnumVariantNames,
)]
pub enum ItemAction {
    #[default]
    None,
    Heal {
        amount: f32,
    },
}

#[derive(Visit, Reflect, Debug, Clone)]
pub struct Item {
    pub stack_size: InheritableVariable<u32>,

    #[visit(optional)]
    pub description: InheritableVariable<String>,

    #[visit(optional)]
    pub name: InheritableVariable<String>,

    #[visit(optional)]
    pub consumable: InheritableVariable<bool>,

    #[visit(optional)]
    pub preview: InheritableVariable<Option<TextureResource>>,

    #[visit(optional)]
    pub associated_weapon: InheritableVariable<Option<ModelResource>>,

    #[visit(optional)]
    pub action: InheritableVariable<ItemAction>,

    #[reflect(hidden)]
    spark: Handle<Node>,

    #[reflect(hidden)]
    spark_size_change_dir: f32,
}

impl Default for Item {
    fn default() -> Self {
        Self {
            spark: Default::default(),
            spark_size_change_dir: 1.0,
            description: Default::default(),
            name: Default::default(),
            consumable: Default::default(),
            stack_size: 1.into(),
            associated_weapon: Default::default(),
            preview: Default::default(),
            action: Default::default(),
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
        // Create spark from code, since it is the same across all items.
        self.spark = SpriteBuilder::new(BaseBuilder::new().with_depth_offset(0.0025))
            .with_size(0.04)
            .with_color(Color::from_rgba(255, 255, 255, 160))
            .with_texture(
                ctx.resource_manager
                    .request::<Texture, _>("data/particles/star_09.png"),
            )
            .build(&mut ctx.scene.graph);

        ctx.scene.graph.link_nodes(self.spark, ctx.handle);

        Level::try_get_mut(ctx.plugins)
            .unwrap()
            .items
            .container
            .push(ctx.handle);
    }

    fn on_deinit(&mut self, context: &mut ScriptDeinitContext) {
        if let Some(level) = Level::try_get_mut(context.plugins) {
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

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}

impl Item {
    pub fn from_resource<F, R>(model_resource: &ModelResource, func: F) -> R
    where
        F: FnOnce(Option<&Item>) -> R,
    {
        let data = model_resource.data_ref();
        let graph = &data.get_scene().graph;
        func(
            graph
                .try_get(graph.get_root())
                .and_then(|n| n.try_get_script::<Item>()),
        )
    }

    pub fn add_to_scene(
        scene: &mut Scene,
        item_resource: ModelResource,
        position: Vector3<f32>,
        adjust_height: bool,
        stack_size: u32,
    ) {
        let position = if adjust_height {
            let mut intersections = Vec::new();
            let ray = Ray::from_two_points(position, position - Vector3::new(0.0, 1000.0, 0.0));
            scene.graph.physics.cast_ray(
                RayCastOptions {
                    ray_origin: Point3::from(ray.origin),
                    ray_direction: ray.dir,
                    max_len: ray.dir.norm(),
                    groups: Default::default(),
                    sort_results: true,
                },
                &mut intersections,
            );

            if let Some(intersection) = intersections.iter().find(|i| {
                // HACK: Check everything but capsules (helps correctly drop items from actors)
                !matches!(
                    scene.graph[i.collider].as_collider().shape(),
                    ColliderShape::Capsule(_)
                )
            }) {
                intersection.position.coords
            } else {
                position
            }
        } else {
            position
        };

        let item = block_on(item_resource.clone()).unwrap().instantiate(scene);

        let item_ref = &mut scene.graph[item];
        item_ref.local_transform_mut().set_position(position);

        if let Some(item_script) = item_ref.try_get_script_mut::<Item>() {
            item_script
                .stack_size
                .set_value_and_mark_modified(stack_size);
        } else {
            Log::err(format!(
                "Asset {} is not an item asset!",
                item_resource.path().display()
            ));
        }
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
