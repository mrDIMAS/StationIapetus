use crate::CollisionGroups;
use rg3d::core::math::lerpf;
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
    light: Handle<Node>,
    reaction_state: Option<ReactionState>,
}

#[derive(Visit)]
pub enum ReactionState {
    HitDetected {
        time_remaining: f32,
        begin_color: Color,
        end_color: Color,
    },
    EnemyKilled {
        time_remaining: f32,
        dilation_factor: f32,
        begin_color: Color,
        end_color: Color,
    },
}

impl Default for ReactionState {
    fn default() -> Self {
        Self::HitDetected {
            time_remaining: 0.0,
            begin_color: Default::default(),
            end_color: Default::default(),
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub enum SightReaction {
    HitDetected,
    EnemyKilled,
}

const NORMAL_COLOR: Color = Color::from_rgba(0, 162, 232, 200);
const NORMAL_RADIUS: f32 = 0.0012;
const ENEMY_KILLED_TIME: f32 = 0.55;
const HIT_DETECTED_TIME: f32 = 0.4;

impl LaserSight {
    pub fn new(scene: &mut Scene, resource_manager: ResourceManager) -> Self {
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
            .with_color(NORMAL_COLOR)
            .build()])
            .with_cast_shadows(false)
            .with_render_path(RenderPath::Forward)
            .build(&mut scene.graph);

        let light;
        let tip = SpriteBuilder::new(BaseBuilder::new().with_visibility(false).with_children(&[{
            light = PointLightBuilder::new(
                BaseLightBuilder::new(BaseBuilder::new())
                    .cast_shadows(false)
                    .with_scatter_enabled(false)
                    .with_color(NORMAL_COLOR),
            )
            .with_radius(0.30)
            .build(&mut scene.graph);
            light
        }]))
        .with_texture(resource_manager.request_texture("data/particles/star_09.png", None))
        .with_color(NORMAL_COLOR)
        .with_size(0.025)
        .build(&mut scene.graph);

        Self {
            ray,
            tip,
            light,
            reaction_state: None,
        }
    }

    pub fn update(
        &mut self,
        scene: &mut Scene,
        position: Vector3<f32>,
        direction: Vector3<f32>,
        ignore_collider: ColliderHandle,
        dt: f32,
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
                .set_scale(Vector3::new(NORMAL_RADIUS, NORMAL_RADIUS, result.toi));

            scene.graph[self.tip]
                .local_transform_mut()
                .set_position(result.position.coords - direction.scale(0.02));
        }

        if let Some(reaction_state) = self.reaction_state.as_mut() {
            match reaction_state {
                ReactionState::HitDetected {
                    time_remaining,
                    begin_color,
                    end_color,
                } => {
                    *time_remaining -= dt;
                    if *time_remaining <= 0.0 {
                        self.reaction_state = None;
                    } else {
                        let t = *time_remaining / HIT_DETECTED_TIME;
                        let color = end_color.lerp(*begin_color, t);
                        self.set_color(&mut scene.graph, color);
                    }
                }
                ReactionState::EnemyKilled {
                    time_remaining,
                    dilation_factor,
                    begin_color,
                    end_color,
                } => {
                    *time_remaining -= dt;
                    if *time_remaining <= 0.0 {
                        self.reaction_state = None;
                    } else {
                        let t = *time_remaining / HIT_DETECTED_TIME;
                        let color = end_color.lerp(*begin_color, t);
                        let dilation_factor = lerpf(1.0, *dilation_factor, t);
                        self.set_color(&mut scene.graph, color);
                        self.dilate(&mut scene.graph, dilation_factor);
                    }
                }
            }
        }
    }

    pub fn set_reaction(&mut self, reaction: SightReaction) {
        self.reaction_state = Some(match reaction {
            SightReaction::HitDetected => ReactionState::HitDetected {
                time_remaining: HIT_DETECTED_TIME,
                begin_color: Color::from_rgba(200, 0, 0, 200),
                end_color: NORMAL_COLOR,
            },
            SightReaction::EnemyKilled => ReactionState::EnemyKilled {
                time_remaining: ENEMY_KILLED_TIME,
                dilation_factor: 1.1,
                begin_color: Color::from_rgba(255, 0, 0, 200),
                end_color: NORMAL_COLOR,
            },
        });
    }

    fn set_color(&self, graph: &mut Graph, color: Color) {
        graph[self.ray].as_mesh_mut().set_color(color);
        graph[self.light].as_light_mut().set_color(color);
        graph[self.tip].as_sprite_mut().set_color(color);
    }

    fn dilate(&self, graph: &mut Graph, factor: f32) {
        let transform = graph[self.ray].local_transform_mut();
        let scale = **transform.scale();
        transform.set_scale(Vector3::new(
            NORMAL_RADIUS * factor,
            NORMAL_RADIUS * factor,
            scale.z,
        ));
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
