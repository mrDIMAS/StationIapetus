use crate::bot::behavior::Action;
use crate::{bot::behavior::BehaviorContext, GameTime};
use fyrox::{
    core::{
        algebra::{UnitQuaternion, Vector3},
        math::SmoothAngle,
        pool::Handle,
        visitor::prelude::*,
    },
    scene::{graph::Graph, node::Node, Scene},
    utils::behavior::{Behavior, Status},
};

#[derive(Debug, PartialEq, Visit)]
pub struct AimOnTarget {
    yaw: SmoothAngle,
    pitch: SmoothAngle,
    spine: Handle<Node>,
}

impl AimOnTarget {
    pub fn new(spine: Handle<Node>) -> Action {
        Action::AimOnTarget(Self {
            spine,
            ..Default::default()
        })
    }
}

impl Default for AimOnTarget {
    fn default() -> Self {
        Self {
            yaw: SmoothAngle {
                angle: f32::NAN, // Nan means undefined.
                target: 0.0,
                speed: 180.0f32.to_radians(),
            },
            pitch: SmoothAngle {
                angle: 0.0,
                target: 0.0,
                speed: 180.0f32.to_radians(),
            },
            spine: Default::default(),
        }
    }
}

impl AimOnTarget {
    fn aim_vertically(
        &mut self,
        look_dir: Vector3<f32>,
        graph: &mut Graph,
        time: GameTime,
        angle_hack: f32,
    ) -> bool {
        let angle = self.pitch.angle();

        self.pitch
            .set_target(
                look_dir.dot(&Vector3::y()).acos() - std::f32::consts::PI / 2.0 + angle_hack,
            )
            .update(time.delta);

        if self.spine.is_some() {
            graph[self.spine]
                .local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::x_axis(), angle));
        }

        self.pitch.at_target()
    }

    fn aim_horizontally(
        &mut self,
        look_dir: Vector3<f32>,
        scene: &mut Scene,
        model: Handle<Node>,
        time: GameTime,
        body: Handle<Node>,
    ) -> bool {
        if self.yaw.angle.is_nan() {
            let local_look = scene.graph[model].look_vector();
            self.yaw.angle = local_look.x.atan2(local_look.z);
        }

        let angle = self.yaw.angle();

        self.yaw
            .set_target(look_dir.x.atan2(look_dir.z))
            .update(time.delta);

        if let Some(body) = scene.graph.try_get_mut(body) {
            body.local_transform_mut()
                .set_rotation(UnitQuaternion::from_axis_angle(&Vector3::y_axis(), angle));
        }

        self.yaw.at_target()
    }
}

impl<'a> Behavior<'a> for AimOnTarget {
    type Context = BehaviorContext<'a>;

    fn tick(&mut self, context: &mut Self::Context) -> Status {
        let look_dir = context.target.clone().unwrap().position
            - context.character.position(&context.scene.graph);

        let aimed_horizontally = self.aim_horizontally(
            look_dir,
            context.scene,
            context.model,
            context.time,
            context.character.body,
        );
        let aimed_vertically = self.aim_vertically(
            look_dir,
            &mut context.scene.graph,
            context.time,
            context.definition.v_aim_angle_hack.to_radians(),
        );

        if aimed_horizontally && aimed_vertically {
            Status::Success
        } else {
            Status::Running
        }
    }
}
