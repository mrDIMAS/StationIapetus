use crate::{
    character::{CharacterMessage, CharacterMessageData},
    weapon::{find_parent_character, Weapon, WeaponMessage, WeaponMessageData},
    CollisionGroups, Item,
};
use fyrox::{
    core::{
        algebra::{Point3, Vector3},
        math::{aabb::AxisAlignedBoundingBox, ray::Ray},
        pool::Handle,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        variable::InheritableVariable,
        visitor::prelude::*,
        TypeUuidProvider,
    },
    event::Event,
    impl_component_provider,
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

#[derive(Visit, Reflect, Debug, Clone)]
pub struct KineticGun {
    weapon: Weapon,
    #[visit(optional)]
    range: InheritableVariable<f32>,
    #[visit(optional)]
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
        }
    }
}

impl_component_provider!(KineticGun, weapon: Weapon, weapon.item: Item);

impl TypeUuidProvider for KineticGun {
    fn type_uuid() -> Uuid {
        uuid!("2351b380-de4c-4b8a-a33f-a3e598e2ada4")
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
    }

    fn on_update(&mut self, ctx: &mut ScriptContext) {
        self.weapon.on_update(ctx);

        if self.is_active {
            let begin = self.weapon.shot_position(&ctx.scene.graph);

            if self.target.is_none() {
                let dir = self
                    .weapon
                    .shot_direction(&ctx.scene.graph)
                    .scale(*self.range);

                let physics = &mut ctx.scene.graph.physics;
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

                for intersection in query_buffer.iter() {
                    if let Some(collider) = ctx.scene.graph.try_get(intersection.collider) {
                        if let Some(rigid_body) = ctx
                            .scene
                            .graph
                            .try_get_of_type::<RigidBody>(collider.parent())
                        {
                            if rigid_body.body_type() == RigidBodyType::Dynamic {
                                self.target = Some(Target {
                                    node: collider.parent(),
                                    grab_point: collider
                                        .global_transform()
                                        .try_inverse()
                                        .map(|inv| {
                                            inv.transform_point(&intersection.position).coords
                                        })
                                        .unwrap_or_default(),
                                    collider: intersection.collider,
                                });
                            }
                        }
                    }
                }
            }

            if let Some(target) = self.target.as_ref() {
                if let Some(collider) = ctx.scene.graph.try_get_of_type::<Collider>(target.collider)
                {
                    let grab_point = collider
                        .global_transform()
                        .transform_point(&Point3::from(target.grab_point))
                        .coords;

                    let aabb = ctx
                        .scene
                        .graph
                        .aabb_of_descendants(target.node)
                        .unwrap_or_else(AxisAlignedBoundingBox::collapsed);

                    if let Some(target_body) = ctx
                        .scene
                        .graph
                        .try_get_mut_of_type::<RigidBody>(target.node)
                    {
                        let velocity = (begin - grab_point)
                            .scale((0.05 / aabb.volume().max(0.0001) * 6.0).clamp(0.0, 2.0));

                        target_body.set_lin_vel(velocity);
                        target_body.set_ang_vel(Default::default());
                        target_body.wake_up();
                    }
                }
            }
        } else {
            self.target = None;
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
                    if let Some(target) = self.target.as_ref() {
                        let aabb = ctx
                            .scene
                            .graph
                            .aabb_of_descendants(target.node)
                            .unwrap_or_else(AxisAlignedBoundingBox::collapsed);

                        let velocity = direction
                            .unwrap_or_else(|| self.weapon.shot_direction(&ctx.scene.graph))
                            .scale(*self.force / (100.0 * aabb.volume().max(0.0001)).max(1.0));

                        if let Some(target_body) = ctx
                            .scene
                            .graph
                            .try_get_mut_of_type::<RigidBody>(target.node)
                        {
                            target_body.set_lin_vel(velocity);
                            target_body.wake_up();
                        }
                    }

                    self.target = None;
                    self.is_active = false;
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

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
