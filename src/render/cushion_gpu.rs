use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use vello::peniko::ImageData;
use vello::wgpu;
use vello::Renderer;

use crate::layout::LayoutRect;
use crate::render::colors;
use crate::render::colors::ColorSettings;
use crate::render::cushion::CushionConfig;
use crate::tree::arena::FileTree;

const INITIAL_INSTANCE_CAPACITY: usize = 16_384;
const CUSHION_TARGET_FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8Unorm;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Uniforms {
    screen_size: [f32; 2],
    ambient: f32,
    diffuse: f32,
    light_dir: [f32; 3],
    fast_mode: u32,
    exclusion_rect: [f32; 4], // x1,y1,x2,y2 in pixels; treemap is skipped inside this region
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RectInstance {
    rect: [f32; 4],
    color: [f32; 4],
    coeffs: [f32; 4],
    info: [f32; 2], // reserved for future visual channels: size_log_norm, age_norm
    _pad: [f32; 2],
}

pub struct CushionGpu {
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    instance_buffer: wgpu::Buffer,
    instance_bind_group_layout: wgpu::BindGroupLayout,
    instance_bind_group: wgpu::BindGroup,
    instance_capacity: usize,
    instance_count: u32,

    target_texture: wgpu::Texture,
    target_view: wgpu::TextureView,
    target_image: ImageData,
    target_width: u32,
    target_height: u32,
}

impl CushionGpu {
    pub fn new(
        device: &wgpu::Device,
        renderer: &mut Renderer,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("cushion.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/cushion.wgsl").into()),
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("cushion uniforms bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let instance_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("cushion instances bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("cushion pipeline layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &instance_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("cushion pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: CUSHION_TARGET_FORMAT,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cushion uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cushion uniforms bg"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cushion instances"),
            size: (INITIAL_INSTANCE_CAPACITY * std::mem::size_of::<RectInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cushion instances bg"),
            layout: &instance_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: instance_buffer.as_entire_binding(),
            }],
        });

        let (target_texture, target_view, target_image) =
            create_target_texture(device, renderer, width.max(1), height.max(1));

        Ok(Self {
            pipeline,
            uniform_buffer,
            uniform_bind_group,
            instance_buffer,
            instance_bind_group_layout,
            instance_bind_group,
            instance_capacity: INITIAL_INSTANCE_CAPACITY,
            instance_count: 0,
            target_texture,
            target_view,
            target_image,
            target_width: width.max(1),
            target_height: height.max(1),
        })
    }

    pub fn image(&self) -> &ImageData {
        &self.target_image
    }

    pub fn resize_target(
        &mut self,
        device: &wgpu::Device,
        renderer: &mut Renderer,
        width: u32,
        height: u32,
    ) {
        let width = width.max(1);
        let height = height.max(1);
        if width == self.target_width && height == self.target_height {
            return;
        }

        renderer.unregister_texture(self.target_image.clone());

        let (texture, view, image) = create_target_texture(device, renderer, width, height);
        self.target_texture = texture;
        self.target_view = view;
        self.target_image = image;
        self.target_width = width;
        self.target_height = height;
    }

    pub fn update_and_render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        layout_rects: &[LayoutRect],
        tree: &FileTree,
        config: &CushionConfig,
        color_settings: &ColorSettings,
        exclusion_rect: [f32; 4],
    ) {
        let mut instances = Vec::with_capacity(layout_rects.len());
        for rect in layout_rects {
            let node = tree.get(rect.node);
            let x = rect.x.max(0.0);
            let y = rect.y.max(0.0);
            let max_w = (self.target_width as f32 - x).max(0.0);
            let max_h = (self.target_height as f32 - y).max(0.0);
            let w = rect.w.min(max_w).max(0.0);
            let h = rect.h.min(max_h).max(0.0);
            if w < 0.5 || h < 0.5 {
                continue;
            }
            let base = if node.is_dir {
                colors::directory_color(&node.name, rect.depth, color_settings)
            } else {
                let ext = if node.extension_id > 0 {
                    tree.extensions
                        .get(node.extension_id as usize)
                        .map(|s| s.as_str())
                        .unwrap_or("")
                } else {
                    ""
                };
                colors::extension_color(ext, color_settings)
            };

            instances.push(RectInstance {
                rect: [x, y, w, h],
                color: [base.r, base.g, base.b, 1.0],
                coeffs: rect.surface,
                info: [((node.size as f32 + 1.0).log10() / 12.0).clamp(0.0, 1.0), 0.0],
                _pad: [0.0, 0.0],
            });
        }

        self.ensure_instance_capacity(device, instances.len());
        if !instances.is_empty() {
            queue.write_buffer(&self.instance_buffer, 0, bytemuck::cast_slice(&instances));
        }
        self.instance_count = instances.len() as u32;

        let mut light = config.light;
        let len = (light[0] * light[0] + light[1] * light[1] + light[2] * light[2]).sqrt();
        if len > 1e-6 {
            light[0] /= len;
            light[1] /= len;
            light[2] /= len;
        } else {
            light = [0.09759001, 0.19518003, 0.9759001];
        }

        let uniforms = Uniforms {
            screen_size: [self.target_width as f32, self.target_height as f32],
            ambient: config.ambient,
            diffuse: config.diffuse,
            light_dir: light,
            fast_mode: if config.fast_lighting { 1 } else { 0 },
            exclusion_rect,
        };
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("cushion encoder"),
        });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("cushion pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.target_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.078,
                            g: 0.086,
                            b: 0.11,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            if self.instance_count > 0 {
                pass.set_pipeline(&self.pipeline);
                pass.set_bind_group(0, &self.uniform_bind_group, &[]);
                pass.set_bind_group(1, &self.instance_bind_group, &[]);
                pass.draw(0..6, 0..self.instance_count);
            }
        }

        queue.submit(Some(encoder.finish()));
    }

    fn ensure_instance_capacity(&mut self, device: &wgpu::Device, required: usize) {
        if required <= self.instance_capacity {
            return;
        }

        let mut new_cap = self.instance_capacity.max(INITIAL_INSTANCE_CAPACITY);
        while new_cap < required {
            new_cap = (new_cap as f32 * 1.5).ceil() as usize;
        }

        self.instance_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("cushion instances"),
            size: (new_cap * std::mem::size_of::<RectInstance>()) as u64,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.instance_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("cushion instances bg"),
            layout: &self.instance_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: self.instance_buffer.as_entire_binding(),
            }],
        });

        self.instance_capacity = new_cap;
    }
}

fn create_target_texture(
    device: &wgpu::Device,
    renderer: &mut Renderer,
    width: u32,
    height: u32,
) -> (wgpu::Texture, wgpu::TextureView, ImageData) {
    let texture = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("cushion target texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: CUSHION_TARGET_FORMAT,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
            | wgpu::TextureUsages::COPY_SRC
            | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
    let image = renderer.register_texture(texture.clone());

    (texture, view, image)
}
