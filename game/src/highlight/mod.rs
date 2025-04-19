use crate::Game;
use fyrox::scene::collider::BitMask;
use fyrox::{
    core::{color::Color, pool::Handle, sstorage::ImmutableString},
    fxhash::FxHashMap,
    graph::SceneGraph,
    renderer::{
        bundle::{ObserverInfo, RenderContext, RenderDataBundleStorage},
        cache::shader::{binding, property, PropertyGroup, RenderMaterial, RenderPassContainer},
        framework::{
            buffer::BufferUsage,
            error::FrameworkError,
            framebuffer::{Attachment, AttachmentKind, GpuFrameBuffer},
            geometry_buffer::GpuGeometryBuffer,
            gpu_texture::{GpuTextureDescriptor, GpuTextureKind, PixelKind},
            server::GraphicsServer,
            GeometryBufferExt,
        },
        make_viewport_matrix, RenderPassStatistics, SceneRenderPass, SceneRenderPassContext,
    },
    scene::{mesh::surface::SurfaceData, node::Node, Scene},
};
use std::{
    any::TypeId,
    cell::RefCell,
    fmt::{Debug, Formatter},
    rc::Rc,
};

#[derive(Clone)]
pub struct HighlightEntry {
    pub color: Color,
    pub auto_remove: bool,
}

pub struct HighlightRenderPass {
    framebuffer: GpuFrameBuffer,
    quad: GpuGeometryBuffer,
    edge_detect_shader: RenderPassContainer,
    flat_shader: RenderPassContainer,
    pub scene_handle: Handle<Scene>,
    pub nodes_to_highlight: FxHashMap<Handle<Node>, HighlightEntry>,
}

impl Debug for HighlightRenderPass {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HighlightRenderPass")
    }
}

impl HighlightRenderPass {
    pub fn new(server: &dyn GraphicsServer, width: usize, height: usize) -> Rc<RefCell<Self>> {
        let width = width.max(1);
        let height = height.max(1);

        let depth_stencil = server
            .create_2d_render_target(PixelKind::D24S8, width, height)
            .unwrap();

        let frame_texture = server
            .create_texture(GpuTextureDescriptor {
                kind: GpuTextureKind::Rectangle { width, height },
                pixel_kind: PixelKind::RGBA8,
                ..Default::default()
            })
            .unwrap();

        let framebuffer = server
            .create_frame_buffer(
                Some(Attachment {
                    kind: AttachmentKind::DepthStencil,
                    texture: depth_stencil,
                }),
                vec![Attachment {
                    kind: AttachmentKind::Color,
                    texture: frame_texture,
                }],
            )
            .unwrap();

        Rc::new(RefCell::new(Self {
            framebuffer,
            quad: GpuGeometryBuffer::from_surface_data(
                &SurfaceData::make_unit_xy_quad(),
                BufferUsage::StaticDraw,
                server,
            )
            .unwrap(),
            edge_detect_shader: RenderPassContainer::from_str(
                server,
                include_str!("edge_detect.shader"),
            )
            .unwrap(),
            flat_shader: RenderPassContainer::from_str(server, include_str!("flat.shader"))
                .unwrap(),
            scene_handle: Default::default(),
            nodes_to_highlight: Default::default(),
        }))
    }
}

impl SceneRenderPass for HighlightRenderPass {
    fn on_ldr_render(
        &mut self,
        ctx: SceneRenderPassContext,
    ) -> Result<RenderPassStatistics, FrameworkError> {
        let mut stats = RenderPassStatistics::default();

        if self.scene_handle != ctx.scene_handle {
            return Ok(stats);
        }

        // Draw selected nodes in the temporary frame buffer first.
        {
            let view_projection = ctx.camera.view_projection_matrix();

            let observer_info = ObserverInfo {
                observer_position: ctx.camera.global_position(),
                z_near: ctx.camera.projection().z_near(),
                z_far: ctx.camera.projection().z_far(),
                view_matrix: ctx.camera.view_matrix(),
                projection_matrix: ctx.camera.projection_matrix(),
            };

            let mut render_batch_storage =
                RenderDataBundleStorage::new_empty(observer_info.clone());

            let frustum = ctx.camera.frustum();
            let mut render_context = RenderContext {
                render_mask: BitMask::all(),
                observer_info: &observer_info,
                frustum: Some(&frustum),
                storage: &mut render_batch_storage,
                graph: &ctx.scene.graph,
                render_pass_name: &Default::default(),
                elapsed_time: ctx.elapsed_time,
                dynamic_surface_cache: ctx.dynamic_surface_cache,
            };

            let mut additional_data_map = FxHashMap::default();

            for (&root_node_handle, entry) in self.nodes_to_highlight.iter() {
                for (node_handle, node) in ctx.scene.graph.traverse_iter(root_node_handle) {
                    node.collect_render_data(&mut render_context);
                    additional_data_map.insert(node_handle, entry.clone());
                }
            }

            render_batch_storage.sort();

            self.framebuffer
                .clear(ctx.viewport, Some(Color::TRANSPARENT), Some(1.0), None);

            for batch in render_batch_storage.bundles.iter() {
                let Ok(geometry) =
                    ctx.geometry_cache
                        .get(ctx.server, &batch.data, batch.time_to_live)
                else {
                    continue;
                };

                for instance in batch.instances.iter() {
                    let color = &additional_data_map
                        .get(&instance.node_handle)
                        .map(|e| e.color)
                        .unwrap_or_default();

                    let wvp = view_projection * instance.world_transform;
                    let color = color.srgb_to_linear_f32();
                    let properties = PropertyGroup::from([
                        property("worldViewProjection", &wvp),
                        property("diffuseColor", &color),
                    ]);
                    let material = RenderMaterial::from([binding("properties", &properties)]);

                    stats += self.flat_shader.run_pass(
                        1,
                        &ImmutableString::new("Primary"),
                        &self.framebuffer,
                        geometry,
                        ctx.viewport,
                        &material,
                        ctx.uniform_buffer_cache,
                        Default::default(),
                        None,
                    )?;
                }
            }
        }

        // Render full screen quad with edge detect shader to draw outline of selected objects.
        {
            let frame_matrix = make_viewport_matrix(ctx.viewport);
            let frame_texture = &self.framebuffer.color_attachments()[0].texture;

            let properties = PropertyGroup::from([property("worldViewProjection", &frame_matrix)]);
            let material = RenderMaterial::from([
                binding(
                    "frameTexture",
                    (frame_texture, &ctx.fallback_resources.linear_clamp_sampler),
                ),
                binding("properties", &properties),
            ]);

            stats += self.edge_detect_shader.run_pass(
                1,
                &ImmutableString::new("Primary"),
                ctx.framebuffer,
                &self.quad,
                ctx.viewport,
                &material,
                ctx.uniform_buffer_cache,
                Default::default(),
                None,
            )?;
        }

        self.nodes_to_highlight.retain(|_, e| !e.auto_remove);

        Ok(stats)
    }

    fn source_type_id(&self) -> TypeId {
        TypeId::of::<Game>()
    }
}
