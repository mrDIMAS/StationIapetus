use crate::utils::create_camera;
use crate::GameTime;
use fyrox::scene::graph::physics::{Intersection, RayCastOptions};
use fyrox::scene::pivot::PivotBuilder;
use fyrox::{
    core::{
        algebra::{Point3, UnitQuaternion, Vector3},
        math::{ray::Ray, Vector3Ext},
        pool::Handle,
        rand::Rng,
        visitor::{Visit, VisitResult, Visitor},
    },
    engine::resource_manager::ResourceManager,
    rand,
    scene::{base::BaseBuilder, graph::Graph, node::Node, transform::TransformBuilder, Scene},
};

#[derive(Default, Visit)]
pub struct CameraController {
    camera_pivot: Handle<Node>,
    camera_hinge: Handle<Node>,
    camera: Handle<Node>,
    camera_offset: Vector3<f32>,
    target_camera_offset: Vector3<f32>,
    shake_offset: Vector3<f32>,
    target_shake_offset: Vector3<f32>,
    shake_timer: f32,
    #[visit(skip)]
    query_buffer: Vec<Intersection>,
}

impl CameraController {
    pub async fn new(resource_manager: ResourceManager, graph: &mut Graph) -> Self {
        let camera_offset = -0.8;

        let camera;
        let camera_hinge;
        let camera_pivot = PivotBuilder::new(BaseBuilder::new().with_children(&[{
            camera_hinge = PivotBuilder::new(
                BaseBuilder::new()
                    .with_local_transform(
                        TransformBuilder::new()
                            .with_local_position(Vector3::new(-0.22, 0.25, 0.0))
                            .build(),
                    )
                    .with_children(&[{
                        camera = create_camera(
                            resource_manager.clone(),
                            Vector3::new(0.0, 0.0, camera_offset),
                            graph,
                            20.0,
                        )
                        .await;
                        camera
                    }]),
            )
            .build(graph);
            camera_hinge
        }]))
        .build(graph);

        Self {
            camera_pivot,
            camera_hinge,
            camera,
            camera_offset: Vector3::new(0.0, 0.0, camera_offset),
            target_camera_offset: Vector3::new(0.0, 0.0, camera_offset),
            shake_offset: Default::default(),
            target_shake_offset: Default::default(),
            shake_timer: 0.0,
            query_buffer: Default::default(),
        }
    }

    pub fn camera(&self) -> Handle<Node> {
        self.camera
    }

    pub fn request_shake_camera(&mut self) {
        self.shake_timer = 0.24;
    }

    pub fn update(
        &mut self,
        position: Vector3<f32>,
        pitch: f32,
        yaw: UnitQuaternion<f32>,
        is_walking: bool,
        is_running: bool,
        is_aiming: bool,
        owner_collider: Handle<Node>,
        scene: &mut Scene,
        time: GameTime,
    ) {
        if is_walking {
            let (kx, ky) = if is_running { (8.0, 13.0) } else { (5.0, 10.0) };

            self.target_camera_offset.x = 0.015 * (time.elapsed as f32 * kx).cos();
            self.target_camera_offset.y = 0.015 * (time.elapsed as f32 * ky).sin();
        } else {
            self.target_camera_offset.x = 0.0;
            self.target_camera_offset.y = 0.0;
        }

        self.target_camera_offset.z = if is_aiming { 0.2 } else { 0.8 };

        self.update_shake(time.delta);
        self.check_occlusion(owner_collider, scene);

        self.target_camera_offset += self.shake_offset;

        self.camera_offset.follow(&self.target_camera_offset, 0.2);

        scene.graph[self.camera_pivot]
            .local_transform_mut()
            .set_rotation(yaw)
            .set_position(position);

        scene.graph[self.camera]
            .local_transform_mut()
            .set_position(Vector3::new(
                self.camera_offset.x,
                self.camera_offset.y,
                -self.camera_offset.z,
            ));

        // Rotate camera hinge - this will make camera move up and down while look at character
        // (well not exactly on character - on characters head)
        scene.graph[self.camera_hinge]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::x_axis(), pitch));
    }

    fn check_occlusion(&mut self, owner_collider: Handle<Node>, scene: &mut Scene) {
        let ray_origin = scene.graph[self.camera_hinge].global_position();
        let ray_end = scene.graph[self.camera].global_position();
        let dir = (ray_end - ray_origin)
            .try_normalize(std::f32::EPSILON)
            .unwrap_or_default()
            .scale(10.0);
        let ray = Ray {
            origin: ray_origin,
            dir,
        };
        scene.graph.physics.cast_ray(
            RayCastOptions {
                ray_origin: Point3::from(ray.origin),
                ray_direction: ray.dir,
                max_len: ray.dir.norm(),
                groups: Default::default(),
                sort_results: true,
            },
            &mut self.query_buffer,
        );

        for result in self.query_buffer.iter() {
            if result.collider != owner_collider {
                let new_offset = (result.toi.min(0.8) - 0.2).max(0.1);
                if new_offset < self.target_camera_offset.z {
                    self.target_camera_offset.z = new_offset;
                }
                break;
            }
        }
    }

    fn update_shake(&mut self, dt: f32) {
        let xy_range = -0.027..0.027;
        let z_range = 0.01..0.05;
        if self.shake_timer > 0.0 {
            self.shake_timer -= dt;
            let mut rnd = rand::thread_rng();
            self.target_shake_offset = Vector3::new(
                rnd.gen_range(xy_range.clone()),
                rnd.gen_range(xy_range),
                rnd.gen_range(z_range),
            );
        } else {
            self.shake_timer = 0.0;
            self.target_shake_offset = Vector3::new(0.0, 0.0, 0.0);
        }
        self.shake_offset.follow(&self.target_shake_offset, 0.5);
    }
}
