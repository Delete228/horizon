use super::*;
use bytemuck::Zeroable;
use wgpu::util::DeviceExt;

#[test]
#[ignore = "requires a Vulkan/Metal/DX12 adapter"]
fn native_model_material_mips_shade_culling_and_depth() {
    let instance = crate::gfx::test_gpu::instance();
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: None,
        source: wgpu::ShaderSource::Wgsl(include_str!("../terrain.wgsl").into()),
    });
    let pipelines = pipelines(
        &device,
        None,
        &shader,
        wgpu::TextureFormat::Rgba8Unorm,
        &[("SPRITE_ARRAYS", 1.0), ("SPRITE_SPLIT", 2048.0)],
    );
    let texture = |size, mip_level_count, format, usage| {
        device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size,
            mip_level_count,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    };
    let sprites = texture(
        wgpu::Extent3d {
            width: 4,
            height: 4,
            depth_or_array_layers: 2,
        },
        3,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    for layer in 0..2 {
        for mip in 0..3 {
            let size = 4 >> mip;
            let rgba = if layer == 0 {
                [255, 255, 255, 255]
            } else {
                [[255, 0, 0, 255], [0, 255, 0, 255], [0, 0, 255, 255]][mip as usize]
            };
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &sprites,
                    mip_level: mip,
                    origin: wgpu::Origin3d {
                        x: 0,
                        y: 0,
                        z: layer,
                    },
                    aspect: wgpu::TextureAspect::All,
                },
                &rgba.repeat((size * size) as usize),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(size * 4),
                    rows_per_image: Some(size),
                },
                wgpu::Extent3d {
                    width: size,
                    height: size,
                    depth_or_array_layers: 1,
                },
            );
        }
    }
    let lightmap = texture(
        wgpu::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        1,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
    );
    let mut light_pixels = [255u8; 16 * 16 * 4];
    light_pixels[15 * 4] = 128; // block 15, sky 0: lightmap dims only red.
    queue.write_texture(
        lightmap.as_image_copy(),
        &light_pixels,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(64),
            rows_per_image: Some(16),
        },
        lightmap.size(),
    );
    let light_view = lightmap.create_view(&Default::default());
    let mut globals = super::super::Globals::zeroed();
    globals.view_proj = glam::Mat4::IDENTITY.to_cols_array_2d();
    globals.fog = [100_000.0, 200_000.0, 100_000.0, 200_000.0];
    let uniform = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::bytes_of(&globals),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
    });
    let sampler = device.create_sampler(&super::super::fluid::SAMPLER);
    let unmipped = device.create_sampler(&wgpu::SamplerDescriptor {
        lod_max_clamp: 0.0,
        ..super::super::fluid::SAMPLER
    });
    let light_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });
    let group_for = |sprite_view: &wgpu::TextureView, i: usize| {
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::TextureView(sprite_view),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: wgpu::BindingResource::Sampler(if i == 1 { &unmipped } else { &sampler }),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: wgpu::BindingResource::TextureView(&light_view),
            },
            wgpu::BindGroupEntry {
                binding: 4,
                resource: wgpu::BindingResource::Sampler(&light_sampler),
            },
        ];
        for binding in 5..12 {
            entries.push(wgpu::BindGroupEntry {
                binding,
                resource: wgpu::BindingResource::TextureView(sprite_view),
            });
        }
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipelines[i].get_bind_group_layout(0),
            entries: &entries,
        })
    };
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    let mut draws = Vec::new();
    let mut quad = |pixel: u32,
                    z: f32,
                    rgba: [u8; 4],
                    mirrored: bool,
                    sprite: u32,
                    span: f32,
                    light: u32,
                    material: usize,
                    normal: [f32; 3]| {
        let width = 2.0 / 15.0;
        let x = -1.0 + pixel as f32 * width;
        let first = vertices.len() as u32;
        let center = glam::Vec3::new(x + width * 0.5, 0.0, 0.0);
        let pose = if mirrored {
            glam::Mat4::from_translation(center)
                * glam::Mat4::from_scale(glam::Vec3::new(-1.0, 1.0, 1.0))
                * glam::Mat4::from_translation(-center)
        } else {
            glam::Mat4::IDENTITY
        };
        for (pos, uv) in [
            ([x, -1.0, z], [0.0, 0.0]),
            ([x + width, -1.0, z], [span, 0.0]),
            ([x + width, 1.0, z], [span, span]),
            ([x, 1.0, z], [0.0, span]),
        ] {
            vertices.push(super::super::Vertex {
                pos: pose
                    .transform_point3(glam::Vec3::from_array(pos))
                    .to_array(),
                uv,
                color: u32::from_le_bytes(rgba),
                layer: sprite << 16 | [42, 43, 44][material],
                light,
                normal: normal[2].to_ne_bytes().map(|b| b as i8),
                glint_uv: [normal[0], normal[1]],
            });
        }
        draws.push((material, indices.len() as u32));
        for i in [0, 1, 2, 0, 2, 3] {
            indices.push(first + i);
        }
    };
    quad(
        0,
        0.8,
        [255, 255, 255, 0],
        false,
        0,
        0.0,
        0xf0f0,
        0,
        [0.0; 3],
    );
    quad(0, 0.7, [255, 0, 0, 255], false, 0, 0.0, 0xf0f0, 0, [0.0; 3]);
    quad(1, 0.8, [255, 0, 0, 25], false, 0, 0.0, 0xf0f0, 1, [0.0; 3]);
    quad(2, 0.8, [255, 0, 0, 26], false, 0, 0.0, 0xf0f0, 1, [0.0; 3]);
    quad(3, 0.8, [255, 0, 0, 127], false, 0, 0.0, 0xf0f0, 2, [0.0; 3]);
    quad(4, 0.8, [0, 255, 0, 128], false, 0, 0.0, 0xf0f0, 2, [0.0; 3]);
    quad(5, 0.6, [255, 0, 0, 255], false, 0, 0.0, 0xf0f0, 0, [0.0; 3]);
    quad(
        5,
        0.599999,
        [0, 255, 0, 255],
        false,
        0,
        0.0,
        0xf0f0,
        0,
        [0.0; 3],
    );
    quad(
        6,
        0.5,
        [255, 255, 0, 255],
        true,
        0,
        0.0,
        0xf0f0,
        0,
        [0.0; 3],
    );
    quad(7, 0.5, [255; 4], false, 1, 1.0, 0xf0f0, 2, [0.0; 3]);
    quad(8, 0.5, [255; 4], false, 1, 1.0, 0xf0f0, 1, [0.0; 3]);
    quad(9, 0.5, [255; 4], false, 0, 0.0, 0xf0f0, 0, [1.0, 0.0, 0.0]);
    quad(10, 0.5, [255; 4], false, 0, 0.0, 0xf0f0, 0, [0.0; 3]);
    quad(11, 0.5, [255; 4], false, 0, 0.0, 0xf0f0, 0, [0.0, 1.0, 0.0]);
    quad(
        12,
        0.5,
        [255; 4],
        false,
        0,
        0.0,
        0xf0f0,
        0,
        [0.0, -1.0, 0.0],
    );
    quad(
        13,
        0.5,
        [200, 100, 50, 255],
        false,
        0,
        0.0,
        0xf000,
        0,
        [0.0; 3],
    );
    quad(
        14,
        0.5,
        [255; 4],
        false,
        0,
        0.0,
        0xf0f0,
        0,
        [0.70710677, 0.0, 0.70710677],
    );
    let buffer = |contents: &[u8], usage| {
        device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents,
            usage,
        })
    };
    let vertex = buffer(bytemuck::cast_slice(&vertices), wgpu::BufferUsages::VERTEX);
    let index = buffer(bytemuck::cast_slice(&indices), wgpu::BufferUsages::INDEX);
    let size = wgpu::Extent3d {
        width: 15,
        height: 1,
        depth_or_array_layers: 1,
    };
    let color = texture(
        size,
        1,
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
    );
    let depth = texture(
        size,
        1,
        wgpu::TextureFormat::Depth32Float,
        wgpu::TextureUsages::RENDER_ATTACHMENT,
    );
    let readback = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 256,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    // Recreate the same sampler/bindgroup with changed mip views, then restore it.
    for (mips, nether) in [(3, false), (1, false), (3, false), (3, true)] {
        globals.time[1] = f32::from(nether);
        queue.write_buffer(&uniform, 0, bytemuck::bytes_of(&globals));
        let view = sprites.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            mip_level_count: Some(mips),
            ..Default::default()
        });
        let groups: [wgpu::BindGroup; 3] = std::array::from_fn(|i| group_for(&view, i));
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let color_view = color.create_view(&Default::default());
            let depth_view = depth.create_view(&Default::default());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLUE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            pass.set_vertex_buffer(0, vertex.slice(..));
            pass.set_index_buffer(index.slice(..), wgpu::IndexFormat::Uint32);
            for &(material, first) in &draws {
                pass.set_pipeline(&pipelines[material]);
                pass.set_bind_group(0, &groups[material], &[]);
                pass.draw_indexed(first..first + 6, 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: None,
                },
            },
            size,
        );
        queue.submit([encoder.finish()]);
        let (tx, rx) = std::sync::mpsc::channel();
        readback.map_async(wgpu::MapMode::Read, .., move |r| tx.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        rx.recv().unwrap().unwrap();
        let pixels = readback.get_mapped_range(..).unwrap();
        let mut expected = [
            [255, 255, 255, 0],
            [0, 0, 255, 255],
            [255, 0, 0, 26],
            [0, 0, 255, 255],
            [0, 255, 0, 128],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 0, 255, 255],
            [255, 0, 0, 255],
            [127, 127, 127, 255],
            [255, 255, 255, 255],
            [255, 255, 255, 255],
            [102, 102, 102, 255],
            [100, 100, 50, 255],
            [146, 146, 146, 255],
        ];
        if mips == 1 {
            expected[7] = [255, 0, 0, 255];
        }
        if nether {
            expected[11] = [226, 226, 226, 255];
            expected[12] = [226, 226, 226, 255];
        }
        for (i, expected) in expected.into_iter().enumerate() {
            let actual = &pixels[i * 4..i * 4 + 4];
            assert!(
                actual
                    .iter()
                    .zip(expected)
                    .all(|(a, b)| i32::from(*a).abs_diff(b) <= 1),
                "pixel {i}: {actual:?} != {expected:?}"
            );
        }
        drop(pixels);
        readback.unmap();
    }
}
