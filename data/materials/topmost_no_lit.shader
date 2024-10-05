(
    name: "TopmostNoLit",
    passes: [
		(
            name: "Forward",
            draw_parameters: DrawParameters(
                cull_face: Some(Back),
                color_write: ColorMask(
                    red: true,
                    green: true,
                    blue: true,
                    alpha: true,
                ),
                depth_write: true,
                stencil_test: None,
                depth_test: Some(Less),
                blend: Some(BlendParameters(
                    func: BlendFunc(
                        sfactor: SrcAlpha,
                        dfactor: OneMinusSrcAlpha,
                        alpha_sfactor: SrcAlpha,
                        alpha_dfactor: OneMinusSrcAlpha,
                    ),
                    equation: BlendEquation(
                        rgb: Add,
                        alpha: Add
                    )
                )),
                stencil_op: StencilOp(
                    fail: Keep,
                    zfail: Keep,
                    zpass: Keep,
                    write_mask: 0xFFFF_FFFF,
                ),
                scissor_box: None
            ),
            vertex_shader:
               r#"
                layout(location = 0) in vec3 vertexPosition;
                layout(location = 1) in vec2 vertexTexCoord;

                layout(std140) uniform FyroxInstanceData {
                    TInstanceData fyrox_instanceData;
                };

                out vec3 position;
                out vec2 texCoord;

                void main()
                {
                    vec4 localPosition = vec4(vertexPosition, 1.0);
                    gl_Position = fyrox_instanceData.worldViewProjection * localPosition;
                    texCoord = vertexTexCoord;
                }
               "#,

           fragment_shader:
               r#"
                out vec4 FragColor;

                in vec2 texCoord;

                void main()
                {
                    FragColor = texture(diffuseTexture, texCoord);
                    gl_FragDepth = gl_FragCoord.z * 0.93;
                }
               "#,
        ),
	],
    properties: [
		  (
            name: "diffuseTexture",
            kind: Sampler(default: None, fallback: White),
        ),
	],
)