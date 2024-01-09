use fyrox::{
    core::{
        rand::Rng,
        reflect::prelude::*,
        uuid::{uuid, Uuid},
        visitor::{Visit, VisitResult, Visitor},
        TypeUuidProvider,
    },
    impl_component_provider,
    rand::thread_rng,
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct AnimatedLight {
    timer: f32,
}

impl_component_provider!(AnimatedLight);

impl TypeUuidProvider for AnimatedLight {
    fn type_uuid() -> Uuid {
        uuid!("95cee406-a30e-4ae4-a017-e0ccae1ca23d")
    }
}

impl ScriptTrait for AnimatedLight {
    fn on_update(&mut self, context: &mut ScriptContext) {
        self.timer -= context.dt;

        if self.timer < 0.0 {
            let node = &mut context.scene.graph[context.handle];
            let new_visibility = !node.visibility();
            node.set_visibility(new_visibility);

            self.timer = thread_rng().gen_range(0.1..0.5);
        }
    }
}
