use crate::Game;
use fyrox::{
    core::{reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*},
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "ca1f0da2-a3e3-4fd3-b1c0-68060d212227")]
#[visit(optional)]
pub struct PointOfInterest;

impl ScriptTrait for PointOfInterest {
    fn on_init(&mut self, context: &mut ScriptContext) {
        context
            .plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .expect("Level must exist!")
            .pois
            .insert(context.handle);
    }

    fn on_deinit(&mut self, context: &mut ScriptDeinitContext) {
        context
            .plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .expect("Level must exist!")
            .pois
            .remove(&context.node_handle);
    }
}
