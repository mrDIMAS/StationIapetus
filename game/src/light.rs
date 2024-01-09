use fyrox::{
    core::{
        rand::Rng,
        reflect::prelude::*,
        type_traits::prelude::*,
        visitor::{Visit, VisitResult, Visitor},
    },
    rand::thread_rng,
    script::{ScriptContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "95cee406-a30e-4ae4-a017-e0ccae1ca23d")]
pub struct AnimatedLight {
    timer: f32,
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
