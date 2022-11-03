use fyrox::{
    core::{
        reflect::prelude::*,
        sstorage::ImmutableString,
        uuid::{uuid, Uuid},
        visitor::prelude::*,
    },
    impl_component_provider,
    material::PropertyValue,
    scene::{mesh::Mesh, node::TypeUuidProvider, sprite::Sprite},
    script::{ScriptContext, ScriptTrait},
    utils::log::Log,
};

impl ShotTrail {
    pub fn new(max_lifetime: f32) -> Self {
        Self {
            lifetime: 0.0,
            max_lifetime,
        }
    }
}

#[derive(Visit, Reflect, Default, Debug, Clone)]
pub struct ShotTrail {
    lifetime: f32,
    max_lifetime: f32,
}

impl_component_provider!(ShotTrail);

impl TypeUuidProvider for ShotTrail {
    fn type_uuid() -> Uuid {
        uuid!("861f0a93-594c-4909-9977-2d9782301c04")
    }
}

impl ScriptTrait for ShotTrail {
    fn on_update(&mut self, context: &mut ScriptContext) {
        self.lifetime = (self.lifetime + context.dt).min(self.max_lifetime);
        let k = 1.0 - self.lifetime / self.max_lifetime;
        let new_alpha = (255.0 * k) as u8;

        let trail_node = &mut context.scene.graph[context.handle];
        if let Some(mesh) = trail_node.cast_mut::<Mesh>() {
            for surface in mesh.surfaces_mut() {
                let mut material = surface.material().lock();
                let color = material
                    .property_ref(&ImmutableString::new("diffuseColor"))
                    .unwrap()
                    .as_color()
                    .unwrap();
                Log::verify(material.set_property(
                    &ImmutableString::new("diffuseColor"),
                    PropertyValue::Color(color.with_new_alpha(new_alpha)),
                ));
            }
        } else if let Some(sprite) = trail_node.cast_mut::<Sprite>() {
            sprite.set_color(sprite.color().with_new_alpha(new_alpha));
        }

        if self.lifetime >= self.max_lifetime {
            context.scene.remove_node(context.handle);
        }
    }

    fn id(&self) -> Uuid {
        Self::type_uuid()
    }
}
