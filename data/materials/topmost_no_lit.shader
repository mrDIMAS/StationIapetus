(
    name: "TopmostNoLit",
    passes: [
        (
            name: "Forward",
            draw_parameters: (
                cull_face: Some(Back),
                color_write: (
                    red: true,
                    green: true,
                    blue: true,
                    alpha: true,
                ),
                depth_write: true,
                stencil_test: None,
                depth_test: Some(Less),
                blend: Some((
                    func: (
                        sfactor: SrcAlpha,
                        dfactor: OneMinusSrcAlpha,
                        alpha_sfactor: SrcAlpha,
                        alpha_dfactor: OneMinusSrcAlpha,
                    ),
                    equation: (
                        rgb: Add,
                        alpha: Add,
                    ),
                )),
                stencil_op: (
                    fail: Keep,
                    zfail: Keep,
                    zpass: Keep,
                    write_mask: 4294967295,
                ),
                scissor_box: None,
            ),
            vertex_shader: "\r\n                layout(location = 0) in vec3 vertexPosition;\r\n                layout(location = 1) in vec2 vertexTexCoord;\r\n\r\n                out vec3 position;\r\n                out vec2 texCoord;\r\n\r\n                void main()\r\n                {\r\n                    vec4 localPosition = vec4(vertexPosition, 1.0);\r\n                    gl_Position = fyrox_instanceData.worldViewProjection * localPosition;\r\n                    texCoord = vertexTexCoord;\r\n                }\r\n               ",
            vertex_shader_line: 37,
            fragment_shader: "\r\n                out vec4 FragColor;\r\n\r\n                in vec2 texCoord;\r\n\r\n                void main()\r\n                {\r\n                    FragColor = texture(diffuseTexture, texCoord);\r\n                    gl_FragDepth = gl_FragCoord.z * 0.93;\r\n                }\r\n               ",
            fragment_shader_line: 53,
        ),
    ],
    resources: [
        (
            name: "diffuseTexture",
            kind: Texture(
                kind: Sampler2D,
                fallback: White,
            ),
            binding: 0,
        ),
        (
            name: "fyrox_instanceData",
            kind: PropertyGroup([
                (
                    name: "worldMatrix",
                    kind: Matrix4(
                        value: (1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
                    ),
                ),
                (
                    name: "worldViewProjection",
                    kind: Matrix4(
                        value: (1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0),
                    ),
                ),
                (
                    name: "blendShapesCount",
                    kind: Int(
                        value: 0,
                    ),
                ),
                (
                    name: "useSkeletalAnimation",
                    kind: Bool(
                        value: false,
                    ),
                ),
                (
                    name: "blendShapesWeights",
                    kind: Vector4Array(
                        value: [],
                        max_len: 32,
                    ),
                ),
            ]),
            binding: 0,
        ),
    ],
    disabled_passes: [],
)