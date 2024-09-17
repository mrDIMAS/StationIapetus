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
                depth_test: true,
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
            ),
            vertex_shader:
               r#"
                layout(location = 0) in vec3 vertexPosition;
                layout(location = 1) in vec2 vertexTexCoord;

                uniform mat4 fyrox_worldViewProjection;

                out vec3 position;
                out vec2 texCoord;

                void main()
                {
                    vec4 localPosition = vec4(vertexPosition, 1.0);
                    gl_Position = fyrox_worldViewProjection * localPosition;
                    texCoord = vertexTexCoord;
                }
               "#,

           fragment_shader:
               r#"
                uniform sampler2D diffuseTexture;

                out vec4 FragColor;

                in vec2 texCoord;

                void main()
                {
                    FragColor = texture(diffuseTexture, texCoord);
                    gl_FragDepth = gl_FragCoord.z * 0.005;
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