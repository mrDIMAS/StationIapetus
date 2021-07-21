use crate::CollisionGroups;
use rg3d::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        arrayvec::ArrayVec,
        color::Color,
        math::ray::Ray,
        pool::Handle,
        visitor::prelude::*,
    },
    engine::{resource_manager::ResourceManager, ColliderHandle},
    physics::geometry::InteractionGroups,
    scene::{
        base::BaseBuilder,
        graph::Graph,
        light::{point::PointLightBuilder, BaseLightBuilder},
        mesh::{
            surface::{SurfaceBuilder, SurfaceData},
            MeshBuilder, RenderPath,
        },
        node::Node,
        physics::RayCastOptions,
        sprite::SpriteBuilder,
        Scene,
    },
};
use std::sync::{Arc, RwLock};

#[derive(Default, Visit)]
pub struct LaserSight {
    ray: Handle<Node>,
    tip: Handle<Node>,
}

impl LaserSight {
    pub fn new(scene: &mut Scene, resource_manager: ResourceManager) -> Self {
        let color = Color::from_rgba(0, 162, 232, 200);

        let ray = MeshBuilder::new(BaseBuilder::new().with_visibility(false))
            .with_surfaces(vec![SurfaceBuilder::new(Arc::new(RwLock::new(
                SurfaceData::make_cylinder(
                    6,
                    1.0,
                    1.0,
                    false,
                    &UnitQuaternion::from_axis_angle(&Vector3::x_axis(), 90.0f32.to_radians())
                        .to_homogeneous(),
                ),
            )))
            .with_color(color)
            .build()])
            .with_cast_shadows(false)
            .with_render_path(RenderPath::Forward)
            .build(&mut scene.graph);

        let tip = SpriteBuilder::new(
            BaseBuilder::new()
                .with_visibility(false)
                .with_children(&[PointLightBuilder::new(
                    BaseLightBuilder::new(BaseBuilder::new())
                        .cast_shadows(false)
                        .with_scatter_enabled(false)
                        .with_color(color),
                )
                .with_radius(0.30)
                .build(&mut scene.graph)]),
        )
        .with_texture(resource_manager.request_texture("data/particles/star_09.png"))
        .with_color(color)
        .with_size(0.025)
        .build(&mut scene.graph);

        Self { ray, tip }
    }

    pub fn update(
        &self,
        scene: &mut Scene,
        position: Vector3<f32>,
        direction: Vector3<f32>,
        ignore_collider: ColliderHandle,
    ) {
        let mut intersections = ArrayVec::<_, 64>::new();

        let ray = &mut scene.graph[self.ray];
        let max_toi = 100.0;

        scene.physics.cast_ray(
            RayCastOptions {
                ray: Ray::new(position, direction.scale(max_toi)),
                max_len: max_toi,
                groups: InteractionGroups::new(0xFFFF, !(CollisionGroups::ActorCapsule as u32)),
                sort_results: true,
            },
            &mut intersections,
        );

        if let Some(result) = intersections
            .into_iter()
            .find(|i| i.collider != ignore_collider)
        {
            ray.local_transform_mut()
                .set_position(position)
                .set_rotation(UnitQuaternion::face_towards(&direction, &Vector3::y()))
                .set_scale(Vector3::new(0.0012, 0.0012, result.toi));

            scene.graph[self.tip]
                .local_transform_mut()
                .set_position(result.position.coords - direction.scale(0.02));
        }
    }

    pub fn set_visible(&self, visibility: bool, graph: &mut Graph) {
        graph[self.tip].set_visibility(visibility);
        graph[self.ray].set_visibility(visibility);
    }

    pub fn clean_up(&mut self, scene: &mut Scene) {
        scene.graph.remove_node(self.ray);
        scene.graph.remove_node(self.tip);
    }
}
