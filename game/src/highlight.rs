use crate::Game;
use fyrox::{
    core::{color::Color, pool::Handle, sstorage::ImmutableString},
    fxhash::FxHashMap,
    graph::SceneGraph,
    renderer::{
        bundle::{ObserverInfo, RenderContext, RenderDataBundleStorage},
        framework::{
            buffer::BufferUsage,
            error::FrameworkError,
            framebuffer::{
                Attachment, AttachmentKind, BufferLocation, FrameBuffer, ResourceBindGroup,
                ResourceBinding,
            },
            geometry_buffer::GeometryBuffer,
            gpu_program::{GpuProgram, UniformLocation},
            gpu_texture::{
                GpuTextureDescriptor, GpuTextureKind, MagnificationFilter, MinificationFilter,
                PixelKind, WrapMode,
            },
            server::GraphicsServer,
            uniform::StaticUniformBuffer,
            BlendFactor, BlendFunc, BlendParameters, CompareFunc, DrawParameters, ElementRange,
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

struct EdgeDetectShader {
    program: Box<dyn GpuProgram>,
    uniform_buffer_binding: usize,
    frame_texture: UniformLocation,
}

impl EdgeDetectShader {
    pub fn new(state: &dyn GraphicsServer) -> Result<Self, FrameworkError> {
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

layout(std140) uniform Uniforms {
    mat4 worldViewProjection;
};

out vec2 texCoord;

void main()
{
    texCoord = vertexTexCoord;
    gl_Position = worldViewProjection * vec4(vertexPosition, 1.0);
}"#;

        let program = state.create_program("EdgeDetectShader", vertex_source, fragment_source)?;
        Ok(Self {
            uniform_buffer_binding: program
                .uniform_block_index(&ImmutableString::new("Uniforms"))?,
            frame_texture: program.uniform_location(&ImmutableString::new("frameTexture"))?,
            program,
        })
    }
}

struct FlatShader {
    program: Box<dyn GpuProgram>,
    uniform_buffer_binding: usize,
}

impl FlatShader {
    pub fn new(server: &dyn GraphicsServer) -> Result<Self, FrameworkError> {
        let fragment_source = r#"
out vec4 FragColor;

layout(std140) uniform Uniforms {
    mat4 worldViewProjection;
    vec4 diffuseColor;
};

void main()
{
    FragColor = diffuseColor;
}"#;

        let vertex_source = r#"
layout(location = 0) in vec3 vertexPosition;

layout(std140) uniform Uniforms {
    mat4 worldViewProjection;
    vec4 diffuseColor;
};

void main()
{
    gl_Position = worldViewProjection * vec4(vertexPosition, 1.0);
}"#;

        let program = server.create_program("FlatShader", vertex_source, fragment_source)?;
        Ok(Self {
            uniform_buffer_binding: program
                .uniform_block_index(&ImmutableString::new("Uniforms"))?,
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
    quad: Box<dyn GeometryBuffer>,
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
    pub fn new(server: &dyn GraphicsServer, width: usize, height: usize) -> Rc<RefCell<Self>> {
        let width = width.max(1);
        let height = height.max(1);

        let depth_stencil = server
            .create_texture(GpuTextureDescriptor {
                kind: GpuTextureKind::Rectangle { width, height },
                pixel_kind: PixelKind::D24S8,
                min_filter: MinificationFilter::Nearest,
                mag_filter: MagnificationFilter::Nearest,
                mip_count: 1,
                s_wrap_mode: WrapMode::ClampToEdge,
                t_wrap_mode: WrapMode::ClampToEdge,
                r_wrap_mode: WrapMode::ClampToEdge,
                anisotropy: 1.0,
                data: None,
            })
            .unwrap();

        let frame_texture = server
            .create_texture(GpuTextureDescriptor {
                kind: GpuTextureKind::Rectangle { width, height },
                pixel_kind: PixelKind::RGBA8,
                min_filter: MinificationFilter::Linear,
                mag_filter: MagnificationFilter::Linear,
                mip_count: 1,
                s_wrap_mode: WrapMode::ClampToEdge,
                t_wrap_mode: WrapMode::ClampToEdge,
                r_wrap_mode: WrapMode::ClampToEdge,
                anisotropy: 1.0,
                data: None,
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
            quad: <dyn GeometryBuffer>::from_surface_data(
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

            let observer_info = ObserverInfo {
                observer_position: ctx.camera.global_position(),
                z_near: ctx.camera.projection().z_near(),
                z_far: ctx.camera.projection().z_far(),
                view_matrix: ctx.camera.view_matrix(),
                projection_matrix: ctx.camera.projection_matrix(),
            };

            let mut render_batch_storage = RenderDataBundleStorage::new_empty(observer_info.clone());

            let frustum = ctx.camera.frustum();
            let mut render_context = RenderContext {
                observer_info: &observer_info,
                frustum: Some(&frustum),
                storage: &mut render_batch_storage,
                graph: &ctx.scene.graph,
                render_pass_name: &Default::default(),
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
                let Some(geometry) =
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
                    let uniform_buffer = ctx.uniform_buffer_cache.write(
                        StaticUniformBuffer::<512>::new()
                            .with(&(view_projection * instance.world_transform))
                            .with(&color.srgb_to_linear_f32()),
                    )?;

                    let shader = &self.flat_shader;
                    self.framebuffer.draw(
                        geometry,
                        ctx.viewport,
                        &*shader.program,
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
                        &[ResourceBindGroup {
                            bindings: &[ResourceBinding::Buffer {
                                buffer: uniform_buffer,
                                binding: BufferLocation::Auto {
                                    shader_location: shader.uniform_buffer_binding,
                                },
                                data_usage: Default::default(),
                            }],
                        }],
                        instance.element_range,
                    )?;
                }
            }
        }

        // Render full screen quad with edge detect shader to draw outline of selected objects.
        {
            let frame_matrix = make_viewport_matrix(ctx.viewport);
            let shader = &self.edge_detect_shader;
            let frame_texture = self.framebuffer.color_attachments()[0].texture.clone();
            ctx.framebuffer.draw(
                &*self.quad,
                ctx.viewport,
                &*shader.program,
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
                &[ResourceBindGroup {
                    bindings: &[
                        ResourceBinding::texture(&frame_texture, &shader.frame_texture),
                        ResourceBinding::Buffer {
                            buffer: ctx
                                .uniform_buffer_cache
                                .write(StaticUniformBuffer::<512>::new().with(&frame_matrix))?,
                            binding: BufferLocation::Auto {
                                shader_location: shader.uniform_buffer_binding,
                            },
                            data_usage: Default::default(),
                        },
                    ],
                }],
                ElementRange::Full,
            )?;
        }

        self.nodes_to_highlight.retain(|_, e| !e.auto_remove);

        Ok(Default::default())
    }

    fn source_type_id(&self) -> TypeId {
        TypeId::of::<Game>()
    }
}
