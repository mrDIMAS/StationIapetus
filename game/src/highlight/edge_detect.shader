(
    name: "EdgeDetect",
    resources: [
        (
            name: "frameTexture",
            kind: Texture(kind: Sampler2D, fallback: White),
            binding: 0
        ),
        (
            name: "properties",
            kind: PropertyGroup([
                (name: "worldViewProjection", kind: Matrix4()),
            ]),
            binding: 0
        ),
    ],
    passes: [
        (
            name: "Primary",

            draw_parameters: DrawParameters(
                cull_face: None,
                color_write: ColorMask(
                    red: true,
                    green: true,
                    blue: true,
                    alpha: true,
                ),
                depth_write: false,
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

                    out vec2 texCoord;

                    void main()
                    {
                        texCoord = vertexTexCoord;
                        gl_Position = properties.worldViewProjection * vec4(vertexPosition, 1.0);
                    }
                "#,

            fragment_shader:
                r#"
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
                    }
                "#,
        )
    ]
)