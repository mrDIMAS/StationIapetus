use crate::Game;
use fyrox::{
    core::{reflect::prelude::*, type_traits::prelude::*, visitor::prelude::*},
    plugin::error::GameResult,
    script::{ScriptContext, ScriptDeinitContext, ScriptTrait},
};

#[derive(Visit, Reflect, Default, Debug, Clone, TypeUuidProvider, ComponentProvider)]
#[type_uuid(id = "9c258713-e44e-4366-a236-f91e09c6f0aa")]
#[visit(optional)]
pub struct DeathZone;

impl ScriptTrait for DeathZone {
    fn on_start(&mut self, ctx: &mut ScriptContext) -> GameResult {
        ctx.plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .unwrap()
            .death_zones
            .insert(ctx.handle);
        Ok(())
    }

    fn on_deinit(&mut self, ctx: &mut ScriptDeinitContext) -> GameResult {
        ctx.plugins
            .get_mut::<Game>()
            .level
            .as_mut()
            .unwrap()
            .death_zones
            .remove(&ctx.node_handle);
        Ok(())
    }
}
