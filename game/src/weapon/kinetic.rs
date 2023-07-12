use crate::{
    character::{CharacterMessage, CharacterMessageData},
    weapon::{find_parent_character, Weapon},
    CollisionGroups, Item,
};
use fyrox::scene::rigidbody::RigidBodyType;
use fyrox::{
    core::{
        algebra::Point3,
        math::ray::Ray,
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
        collider::{BitMask, InteractionGroups},
        graph::physics::RayCastOptions,
        node::Node,
        rigidbody::RigidBody,
    },
    script::{
        ScriptContext, ScriptDeinitContext, ScriptMessageContext, ScriptMessagePayload, ScriptTrait,
    },
};

#[derive(Visit, Reflect, Debug, Clone)]
pub struct KineticGun {
    weapon: Weapon,
    range: InheritableVariable<f32>,
    #[reflect(hidden)]
    is_active: bool,
    #[reflect(hidden)]
    target: Handle<Node>,
}

impl Default for KineticGun {
    fn default() -> Self {
        Self {
            weapon: Default::default(),
            is_active: false,
            range: 10.0.into(),
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
                                self.target = collider.parent();
                            }
                        }
                    }
                }
            }

            if let Some(target_body) = ctx
                .scene
                .graph
                .try_get_mut_of_type::<RigidBody>(self.target)
            {
                let force = (begin - target_body.global_position())
                    .normalize()
                    .scale(10.0);

                target_body.apply_force(force);
                target_body.wake_up();
            }
        } else {
            self.target = Handle::NONE;
        }
    }

    fn on_message(
        &mut self,
        message: &mut dyn ScriptMessagePayload,
        ctx: &mut ScriptMessageContext,
    ) {
        self.weapon.on_message(message, ctx);

        if let Some(character_message) = message.downcast_ref::<CharacterMessage>() {
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
