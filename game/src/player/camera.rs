use crate::Player;
use fyrox::{
    core::{
        algebra::{Point3, UnitQuaternion, Vector3},
        inspect::prelude::*,
        math::{ray::Ray, Vector3Ext},
        pool::Handle,
        rand::Rng,
        reflect::Reflect,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    impl_component_provider, rand,
    scene::{
        graph::{
            map::NodeHandleMap,
            physics::{Intersection, RayCastOptions},
        },
        node::{Node, TypeUuidProvider},
        Scene,
    },
    script::{ScriptContext, ScriptTrait},
};

#[derive(Default, Visit, Reflect, Inspect, Debug, Clone)]
pub struct CameraController {
    player: Handle<Node>,
    ignorable_collider: Handle<Node>,
    camera_hinge: Handle<Node>,
    camera: Handle<Node>,
    camera_offset: Vector3<f32>,
    target_camera_offset: Vector3<f32>,
    shake_offset: Vector3<f32>,
    target_shake_offset: Vector3<f32>,
    shake_timer: f32,
    #[visit(skip)]
    #[reflect(hidden)]
    #[inspect(skip)]
    query_buffer: Vec<Intersection>,
}

impl CameraController {
    pub fn camera(&self) -> Handle<Node> {
        self.camera
    }

    pub fn request_shake_camera(&mut self) {
        self.shake_timer = 0.24;
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

impl_component_provider!(CameraController);

impl TypeUuidProvider for CameraController {
    fn type_uuid() -> Uuid {
        uuid!("a4681191-0b6f-4398-891d-c5b44019fb31")
    }
}

impl ScriptTrait for CameraController {
    fn on_update(&mut self, context: &mut ScriptContext) {
        let (is_walking, is_running, is_aiming, yaw, pitch) = context
            .scene
            .graph
            .try_get(self.player)
            .and_then(|p| p.try_get_script::<Player>())
            .map(|p| {
                (
                    p.is_walking(),
                    p.is_running(context.scene),
                    p.is_aiming(),
                    p.controller.yaw,
                    p.controller.pitch,
                )
            })
            .unwrap_or_default();

        if is_walking {
            let (kx, ky) = if is_running { (8.0, 13.0) } else { (5.0, 10.0) };

            self.target_camera_offset.x = 0.015 * (context.elapsed_time * kx).cos();
            self.target_camera_offset.y = 0.015 * (context.elapsed_time * ky).sin();
        } else {
            self.target_camera_offset.x = 0.0;
            self.target_camera_offset.y = 0.0;
        }

        self.target_camera_offset.z = if is_aiming { 0.2 } else { 0.8 };

        self.update_shake(context.dt);
        self.check_occlusion(self.ignorable_collider, context.scene);

        self.target_camera_offset += self.shake_offset;

        self.camera_offset.follow(&self.target_camera_offset, 0.2);

        context.scene.graph[context.handle]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), yaw));

        context.scene.graph[self.camera]
            .local_transform_mut()
            .set_position(Vector3::new(
                self.camera_offset.x,
                self.camera_offset.y,
                -self.camera_offset.z,
            ));

        // Rotate camera hinge - this will make camera move up and down while look at character
        // (well not exactly on character - on characters head)
        context.scene.graph[self.camera_hinge]
            .local_transform_mut()
            .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::x_axis(), pitch));
    }

    fn remap_handles(&mut self, old_new_mapping: &NodeHandleMap) {
        old_new_mapping
            .map(&mut self.ignorable_collider)
            .map(&mut self.camera)
            .map(&mut self.player)
            .map(&mut self.camera_hinge);
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}