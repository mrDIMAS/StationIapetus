use crate::{
    character::{CharacterMessage, CharacterMessageData},
    highlight::HighlightEntry,
    player::{camera::CameraController, Player},
    weapon::{find_parent_character, Weapon, WeaponMessage, WeaponMessageData},
    CollisionGroups, Game, Item,
};
use fyrox::graph::SceneGraph;
use fyrox::scene::graph::Graph;
use fyrox::{
    core::{
        algebra::{Point3, UnitQuaternion, UnitVector3, Vector3},
        color::Color,
        impl_component_provider,
        math::{self, aabb::AxisAlignedBoundingBox, ray::Ray},
        pool::Handle,
        reflect::prelude::*,
        type_traits::prelude::*,
        variable::InheritableVariable,
        visitor::prelude::*,
    },
    event::{Event, WindowEvent},
    keyboard::{KeyCode, PhysicalKey},
    scene::{
        collider::{BitMask, Collider, InteractionGroups},
        graph::physics::RayCastOptions,
        node::Node,
        rigidbody::{RigidBody, RigidBodyType},
    },
    script::{
        ScriptContext, ScriptDeinitContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait,
    },
};

#[derive(Visit, Reflect, Debug, Default, Clone)]
struct Target {
    grab_point: Vector3<f32>,
    node: Handle<Node>,
    collider: Handle<Node>,
}

#[derive(Visit, Reflect, Debug, Clone, TypeUuidProvider)]
#[type_uuid(id = "2351b380-de4c-4b8a-a33f-a3e598e2ada4")]
#[visit(optional)]
pub struct KineticGun {
    weapon: Weapon,
    ray: InheritableVariable<Handle<Node>>,
    laser_sight: InheritableVariable<Handle<Node>>,
    range: InheritableVariable<f32>,
    force: InheritableVariable<f32>,
    #[reflect(hidden)]
    is_active: bool,
    #[reflect(read_only)]
    target: Option<Target>,
}

impl Default for KineticGun {
    fn default() -> Self {
        Self {
            weapon: Default::default(),
            is_active: false,
            range: 10.0.into(),
            force: 10.0.into(),
            target: Default::default(),
            ray: Default::default(),
            laser_sight: Default::default(),
        }
    }
}

impl_component_provider!(KineticGun, weapon: Weapon, weapon.item: Item);

impl KineticGun {
    fn reset_target(&mut self, game: &mut Game) {
        if let Some(target) = self.target.take() {
            if let Some(highlighter) = game.highlighter.as_mut() {
                let mut highlighter = highlighter.borrow_mut();

                highlighter.nodes_to_highlight.remove(&target.node);
            }
        }
    }

    fn try_pick_target(&self, graph: &mut Graph) -> Result<Target, Handle<Node>> {
        let begin = self.weapon.shot_position(graph);
        let dir = self.weapon.shot_direction(graph).scale(*self.range);

        let physics = &mut graph.physics;
        let ray = Ray::new(begin, dir);

        let mut query_buffer = Vec::default();
        physics.cast_ray(
            RayCastOptions {
                ray_origin: Point3::from(ray.origin),
                ray_direction: ray.dir,
                max_len: ray.dir.norm(),
                groups: InteractionGroups::new(
                    BitMask(0xFFFF),
                    // Prevent characters from grabbing.
                    BitMask(!(CollisionGroups::ActorCapsule as u32)),
                ),
                sort_results: true,
            },
            &mut query_buffer,
        );

        if let Some(intersection) = query_buffer.first() {
            if let Some(collider) = graph.try_get(intersection.collider) {
                if let Some(rigid_body) = graph.try_get_of_type::<RigidBody>(collider.parent()) {
                    if rigid_body.body_type() == RigidBodyType::Dynamic {
                        let potential_target_node = collider.parent();

                        let aabb = graph
                            .aabb_of_descendants(potential_target_node, |_, _| true)
                            .unwrap_or_else(AxisAlignedBoundingBox::collapsed);

                        return if aabb.volume() <= 0.15 {
                            Ok(Target {
                                node: potential_target_node,
                                grab_point: collider
                                    .global_transform()
                                    .try_inverse()
                                    .map(|inv| inv.transform_point(&intersection.position).coords)
                                    .unwrap_or_default(),
                                collider: intersection.collider,
                            })
                        } else {
                            Err(potential_target_node)
                        };
                    }
                }
            }
        }

        Err(Handle::NONE)
    }
}

impl ScriptTrait for KineticGun {
    fn on_init(&mut self, ctx: &mut ScriptContext) {
        self.weapon.on_init(ctx);
    }

    fn on_start(&mut self, ctx: &mut ScriptContext) {
        self.weapon.on_start(ctx);

        ctx.message_dispatcher
            .subscribe_to::<CharacterMessage>(ctx.handle);
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) {
        self.weapon.on_deinit(ctx);
    }

    fn on_os_event(&mut self, event: &Event<()>, ctx: &mut ScriptContext) {
        self.weapon.on_os_event(event, ctx);

        if let Some(level) = ctx.plugins.get::<Game>().level.as_ref() {
            if let Some(target) = self.target.as_ref() {
                if let Event::WindowEvent {
                    event: WindowEvent::KeyboardInput { event, .. },
                    ..
                } = event
                {
                    if let Some(player) = ctx
                        .scene
                        .graph
                        .try_get_script_component_of::<Player>(level.player)
                    {
                        if let Some(camera_controller) = ctx
                            .scene
                            .graph
                            .try_get_script_component_of::<CameraController>(
                                player.camera_controller,
                            )
                        {
                            if let Some(camera) = ctx.scene.graph.try_get(camera_controller.camera)
                            {
                                if let PhysicalKey::Code(key) = event.physical_key {
                                    let axis = match key {
                                        KeyCode::KeyZ => Some(camera.side_vector()),
                                        KeyCode::KeyX => Some(-camera.side_vector()),
                                        KeyCode::KeyC => Some(-camera.look_vector()),
                                        KeyCode::KeyV => Some(camera.look_vector()),
                                        _ => None,
                                    };
                                    if let Some(axis) = axis {
                                        let rotation = UnitQuaternion::from_axis_angle(
                                            &UnitVector3::new_normalize(axis),
                                            5.0f32.to_radians(),
                                        );

                                        if let Some(target_body) = ctx
                                            .scene
                                            .graph
                                            .try_get_mut_of_type::<RigidBody>(target.node)
                                        {
                                            let local_transform = target_body.local_transform_mut();
                                            let new_rotation =
                                                **local_transform.rotation() * rotation;
                                            local_transform.set_rotation(new_rotation);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        self.weapon.on_update(ctx);

        if self.is_active {
            let begin = self.weapon.shot_position(&ctx.scene.graph);

            if let Some(target) = self.target.as_ref() {
                if let Some(collider) = ctx.scene.graph.try_get_of_type::<Collider>(target.collider)
                {
                    if collider.is_globally_enabled() {
                        let grab_point = collider
                            .global_transform()
                            .transform_point(&Point3::from(target.grab_point))
                            .coords;

                        let delta = begin - grab_point;

                        let relative_delta = &ctx.scene.graph[ctx.handle]
                            .global_transform()
                            .try_inverse()
                            .unwrap_or_default()
                            .transform_vector(&delta)
                            .try_normalize(f32::EPSILON)
                            .unwrap_or_default();

                        if let Some(ray) = ctx.scene.graph.try_get_mut(*self.ray) {
                            let rotation = math::vector_to_quat(-*relative_delta);

                            ray.local_transform_mut()
                                .set_rotation(rotation)
                                .set_scale(Vector3::new(1.0, 1.0, delta.norm()));
                        }

                        if let Some(target_body) = ctx
                            .scene
                            .graph
                            .try_get_mut_of_type::<RigidBody>(target.node)
                        {
                            let velocity = delta.scale(2.0);

                            target_body.set_lin_vel(velocity);
                            target_body.set_ang_vel(Default::default());
                            target_body.wake_up();
                        }
                    } else {
                        self.reset_target(ctx.plugins.get_mut::<Game>());
                    }
                }
            } else if let Some(highlighter) = ctx.plugins.get_mut::<Game>().highlighter.as_mut() {
                match self.try_pick_target(&mut ctx.scene.graph) {
                    Err(inappropriate_target) => {
                        if inappropriate_target.is_some() {
                            highlighter.borrow_mut().nodes_to_highlight.insert(
                                inappropriate_target,
                                HighlightEntry {
                                    color: Color::RED,
                                    auto_remove: true,
                                },
                            );
                        }
                    }
                    Ok(target) => {
                        highlighter.borrow_mut().nodes_to_highlight.insert(
                            target.node,
                            HighlightEntry {
                                color: Color::GREEN,
                                auto_remove: true,
                            },
                        );
                    }
                }
            }
        } else {
            self.reset_target(ctx.plugins.get_mut::<Game>());
        }

        if let Some(ray) = ctx.scene.graph.try_get_mut(*self.ray) {
            ray.set_visibility(self.target.is_some());
        }

        if let Some(laser_sight) = ctx.scene.graph.try_get_mut(*self.laser_sight) {
            laser_sight.set_visibility(self.target.is_none());
        }
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        self.weapon.on_message(message, ctx);

        if let Some(msg) = message.downcast_ref::<WeaponMessage>() {
            if msg.weapon == ctx.handle {
                if let WeaponMessageData::Shoot { direction } = msg.data {
                    match self.target.as_ref() {
                        Some(target) => {
                            let velocity = direction
                                .unwrap_or_else(|| self.weapon.shot_direction(&ctx.scene.graph))
                                .scale(*self.force);

                            if let Some(target_body) = ctx
                                .scene
                                .graph
                                .try_get_mut_of_type::<RigidBody>(target.node)
                            {
                                target_body.set_lin_vel(velocity);
                                target_body.wake_up();
                            }

                            self.reset_target(ctx.plugins.get_mut::<Game>());
                        }
                        None => {
                            if let Ok(new_target) = self.try_pick_target(&mut ctx.scene.graph) {
                                if let Some(highlighter) =
                                    ctx.plugins.get_mut::<Game>().highlighter.as_mut()
                                {
                                    highlighter.borrow_mut().nodes_to_highlight.insert(
                                        new_target.node,
                                        HighlightEntry {
                                            color: Color::GREEN,
                                            auto_remove: false,
                                        },
                                    );
                                }

                                self.target = Some(new_target);
                            }
                        }
                    }
                }
            }
        } else if let Some(character_message) = message.downcast_ref::<CharacterMessage>() {
            if let Some((parent_character_handle, _)) =
                find_parent_character(ctx.handle, &ctx.scene.graph)
            {
                match character_message.data {
                    CharacterMessageData::BeganAiming
                        if character_message.character == parent_character_handle =>
                    {
                        self.is_active = true;
                    }
                    CharacterMessageData::EndedAiming
                        if character_message.character == parent_character_handle =>
                    {
                        self.is_active = false;
                    }
                    _ => (),
                }
            }
        }
    }
}
