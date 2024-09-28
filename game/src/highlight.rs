use crate::Game;
use fyrox::renderer::framework::buffer::BufferUsage;
use fyrox::renderer::framework::state::GraphicsServer;
use fyrox::renderer::framework::{CompareFunc, GeometryBufferExt};
use fyrox::{
    core::{
        algebra::{Matrix4, Vector3},
        color::Color,
        pool::Handle,
        sstorage::ImmutableString,
    },
    fxhash::FxHashMap,
    graph::{BaseSceneGraph, SceneGraph},
    renderer::{
        bundle::{RenderContext, RenderDataBundleStorage},
        framework::{
            error::FrameworkError,
            framebuffer::{Attachment, AttachmentKind, FrameBuffer},
            geometry_buffer::GeometryBuffer,
            gpu_program::{GpuProgram, UniformLocation},
            gpu_texture::{
                Coordinate, GpuTextureKind, MagnificationFilter, MinificationFilter, PixelKind,
                WrapMode,
            },
            state::GlGraphicsServer,
            BlendFactor, BlendFunc, BlendParameters, DrawParameters, ElementRange,
        },
        RenderPassStatistics, SceneRenderPass, SceneRenderPassContext,
    },
    scene::{mesh::surface::SurfaceData, node::Node, Scene},
};
use std::fmt::{Debug, Formatter};
use std::{any::TypeId, cell::RefCell, rc::Rc};

struct EdgeDetectShader {
    program: GpuProgram,
    wvp_matrix: UniformLocation,
    frame_texture: UniformLocation,
}

impl EdgeDetectShader {
    pub fn new(state: &GlGraphicsServer) -> Result<Self, FrameworkError> {
        let fragment_source = r#"
uniform sampler2D frameTexture;

in vec2 texCoord;

out vec4 FragColor;

void main() {
	ivec2 size = textureSize(frameTexture, 0);

	float w = 1.0 / float(size.x);
	float h = 1.0 / float(size.y);

    vec4 n[9];
	n[0] = texture(frameTexture, texCoord + vec2(-w, -h));
	n[1] = texture(frameTexture, texCoord + vec2(0.0, -h));
	n[2] = texture(frameTexture, texCoord + vec2(w, -h));
	n[3] = texture(frameTexture, texCoord + vec2( -w, 0.0));
	n[4] = texture(frameTexture, texCoord);
	n[5] = texture(frameTexture, texCoord + vec2(w, 0.0));
	n[6] = texture(frameTexture, texCoord + vec2(-w, h));
	n[7] = texture(frameTexture, texCoord + vec2(0.0, h));
	n[8] = texture(frameTexture, texCoord + vec2(w, h));

	vec4 sobel_edge_h = n[2] + (2.0 * n[5]) + n[8] - (n[0] + (2.0 * n[3]) + n[6]);
  	vec4 sobel_edge_v = n[0] + (2.0 * n[1]) + n[2] - (n[6] + (2.0 * n[7]) + n[8]);
	vec4 sobel = sqrt((sobel_edge_h * sobel_edge_h) + (sobel_edge_v * sobel_edge_v));

	FragColor = vec4(sobel.rgb, (sobel.r + sobel.g + sobel.b) * 0.333);
}"#;

        let vertex_source = r#"
layout(location = 0) in vec3 vertexPosition;
layout(location = 1) in vec2 vertexTexCoord;

uniform mat4 worldViewProjection;

out vec2 texCoord;

void main()
{
    texCoord = vertexTexCoord;
    gl_Position = worldViewProjection * vec4(vertexPosition, 1.0);
}"#;

        let program =
            GpuProgram::from_source(state, "EdgeDetectShader", vertex_source, fragment_source)?;
        Ok(Self {
            wvp_matrix: program
                .uniform_location(state, &ImmutableString::new("worldViewProjection"))?,
            frame_texture: program
                .uniform_location(state, &ImmutableString::new("frameTexture"))?,
            program,
        })
    }
}

struct FlatShader {
    program: GpuProgram,
    wvp_matrix: UniformLocation,
    diffuse_color: UniformLocation,
}

impl FlatShader {
    pub fn new(server: &GlGraphicsServer) -> Result<Self, FrameworkError> {
        let fragment_source = r#"
out vec4 FragColor;

uniform vec4 diffuseColor;

void main()
{
    FragColor = diffuseColor;
}"#;

        let vertex_source = r#"
layout(location = 0) in vec3 vertexPosition;

uniform mat4 worldViewProjection;

void main()
{
    gl_Position = worldViewProjection * vec4(vertexPosition, 1.0);
}"#;

        let program =
            GpuProgram::from_source(server, "FlatShader", vertex_source, fragment_source)?;
        Ok(Self {
            wvp_matrix: program
                .uniform_location(server, &ImmutableString::new("worldViewProjection"))?,
            diffuse_color: program
                .uniform_location(server, &ImmutableString::new("diffuseColor"))?,
            program,
        })
    }
}

#[derive(Clone)]
pub struct HighlightEntry {
    pub color: Color,
    pub auto_remove: bool,
}

pub struct HighlightRenderPass {
    framebuffer: Box<dyn FrameBuffer>,
    quad: GeometryBuffer,
    edge_detect_shader: EdgeDetectShader,
    flat_shader: FlatShader,
    pub scene_handle: Handle<Scene>,
    pub nodes_to_highlight: FxHashMap<Handle<Node>, HighlightEntry>,
}

impl Debug for HighlightRenderPass {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "HighlightRenderPass")
    }
}

impl HighlightRenderPass {
    pub fn new(server: &GlGraphicsServer, width: usize, height: usize) -> Rc<RefCell<Self>> {
        let depth_stencil = server
            .create_texture(
                GpuTextureKind::Rectangle { width, height },
                PixelKind::D24S8,
                MinificationFilter::Nearest,
                MagnificationFilter::Nearest,
                1,
                None,
            )
            .unwrap();
        depth_stencil
            .borrow_mut()
            .set_wrap(Coordinate::S, WrapMode::ClampToEdge);
        depth_stencil
            .borrow_mut()
            .set_wrap(Coordinate::T, WrapMode::ClampToEdge);

        let frame_texture = server
            .create_texture(
                GpuTextureKind::Rectangle { width, height },
                PixelKind::RGBA8,
                MinificationFilter::Linear,
                MagnificationFilter::Linear,
                1,
                None,
            )
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
            quad: GeometryBuffer::from_surface_data(
                &SurfaceData::make_unit_xy_quad(),
                BufferUsage::StaticDraw,
                server,
            )
            .unwrap(),
            edge_detect_shader: EdgeDetectShader::new(server).unwrap(),
            flat_shader: FlatShader::new(server).unwrap(),
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
        if self.scene_handle != ctx.scene_handle {
            return Ok(Default::default());
        }

        // Draw selected nodes in the temporary frame buffer first.
        {
            let view_projection = ctx.camera.view_projection_matrix();

            let mut render_batch_storage = RenderDataBundleStorage::default();

            let frustum = ctx.camera.frustum();
            let mut render_context = RenderContext {
                observer_position: &ctx.camera.global_position(),
                z_near: ctx.camera.projection().z_near(),
                z_far: ctx.camera.projection().z_far(),
                view_matrix: &ctx.camera.view_matrix(),
                projection_matrix: &ctx.camera.projection_matrix(),
                frustum: Some(&frustum),
                storage: &mut render_batch_storage,
                graph: &ctx.scene.graph,
                render_pass_name: &Default::default(),
            };

            let mut additional_data_map = FxHashMap::default();

            for (&root_node_handle, entry) in self.nodes_to_highlight.iter() {
                for node_handle in ctx.scene.graph.traverse_handle_iter(root_node_handle) {
                    if let Some(node) = ctx.scene.graph.try_get(node_handle) {
                        node.collect_render_data(&mut render_context);

                        additional_data_map.insert(node_handle, entry.clone());
                    }
                }
            }

            render_batch_storage.sort();

            self.framebuffer
                .clear(ctx.viewport, Some(Color::TRANSPARENT), Some(1.0), None);

            for batch in render_batch_storage.bundles.iter() {
                let Some(geometry) =
                    ctx.geometry_cache
                        .get(ctx.server, &batch.data, batch.time_to_live)
                else {
                    continue;
                };

                for instance in batch.instances.iter() {
                    let shader = &self.flat_shader;
                    self.framebuffer.draw(
                        geometry,
                        ctx.viewport,
                        &shader.program,
                        &DrawParameters {
                            cull_face: None,
                            color_write: Default::default(),
                            depth_write: true,
                            stencil_test: None,
                            depth_test: Some(CompareFunc::Less),
                            blend: None,
                            stencil_op: Default::default(),
                            scissor_box: None,
                        },
                        instance.element_range,
                        &mut |mut program_binding| {
                            program_binding
                                .set_matrix4(
                                    &shader.wvp_matrix,
                                    &(view_projection * instance.world_transform),
                                )
                                .set_linear_color(
                                    &shader.diffuse_color,
                                    &additional_data_map
                                        .get(&instance.node_handle)
                                        .map(|e| e.color)
                                        .unwrap_or_default(),
                                );
                        },
                    )?;
                }
            }
        }

        // Render full screen quad with edge detect shader to draw outline of selected objects.
        {
            let frame_matrix = Matrix4::new_orthographic(
                0.0,
                ctx.viewport.w() as f32,
                ctx.viewport.h() as f32,
                0.0,
                -1.0,
                1.0,
            ) * Matrix4::new_nonuniform_scaling(&Vector3::new(
                ctx.viewport.w() as f32,
                ctx.viewport.h() as f32,
                0.0,
            ));
            let shader = &self.edge_detect_shader;
            let frame_texture = self.framebuffer.color_attachments()[0].texture.clone();
            ctx.framebuffer.draw(
                &self.quad,
                ctx.viewport,
                &shader.program,
                &DrawParameters {
                    cull_face: None,
                    color_write: Default::default(),
                    depth_write: false,
                    stencil_test: None,
                    depth_test: Some(CompareFunc::Less),
                    blend: Some(BlendParameters {
                        func: BlendFunc::new(BlendFactor::SrcAlpha, BlendFactor::OneMinusSrcAlpha),
                        ..Default::default()
                    }),
                    stencil_op: Default::default(),
                    scissor_box: None,
                },
                ElementRange::Full,
                &mut |mut program_binding| {
                    program_binding
                        .set_matrix4(&shader.wvp_matrix, &frame_matrix)
                        .set_texture(&shader.frame_texture, &frame_texture);
                },
            )?;
        }

        self.nodes_to_highlight.retain(|_, e| !e.auto_remove);

        Ok(Default::default())
    }

    fn source_type_id(&self) -> TypeId {
        TypeId::of::<Game>()
    }
}
