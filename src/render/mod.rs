pub mod colors;
pub mod cushion;
pub mod cushion_gpu;
pub mod scene;
pub mod text;

use std::sync::Arc;

use anyhow::Result;
use vello::wgpu;
use vello::{AaConfig, RenderParams, Renderer, RendererOptions, Scene};
use winit::window::Window;

use crate::layout::LayoutRect;
use crate::render::colors::ColorSettings;
use crate::tree::arena::FileTree;
use cushion::CushionConfig;
use cushion_gpu::CushionGpu;

/// Holds all GPU rendering state.
pub struct RenderState {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub surface: wgpu::Surface<'static>,
    pub surface_config: wgpu::SurfaceConfiguration,
    pub renderer: Renderer,
    scene_target: wgpu::Texture,
    scene_target_view: wgpu::TextureView,
    blitter: wgpu::util::TextureBlitter,
    cushion_gpu: CushionGpu,
}

impl RenderState {
    /// Initialize the GPU rendering pipeline.
    pub async fn new(window: Arc<Window>) -> Result<Self> {
        let mut instance_desc = wgpu::InstanceDescriptor::default();
        #[cfg(windows)]
        {
            // Prefer DX12 on Windows; Vulkan path has stricter storage-format support on some drivers.
            instance_desc.backends = wgpu::Backends::DX12;
        }
        let instance = wgpu::Instance::new(&instance_desc);

        let surface = instance.create_surface(window.clone())?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await?;

        let device = Arc::new(device);
        let queue = Arc::new(queue);

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .first()
            .copied()
            .ok_or_else(|| anyhow::anyhow!("Surface reported no supported formats"))?;

        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let mut renderer = Renderer::new(
            &device,
            RendererOptions {
                use_cpu: false,
                antialiasing_support: vello::AaSupport::all(),
                num_init_threads: None,
                pipeline_cache: None,
            },
        )?;

        // Vello always renders to an Rgba8Unorm storage image; then we blit to swapchain format.
        let scene_target = create_scene_target(&device, surface_config.width, surface_config.height);
        let scene_target_view = scene_target.create_view(&wgpu::TextureViewDescriptor::default());
        let blitter = wgpu::util::TextureBlitter::new(&device, format);

        let cushion_gpu = CushionGpu::new(
            &device,
            &mut renderer,
            surface_config.width,
            surface_config.height,
        )?;

        Ok(Self {
            device,
            queue,
            surface,
            surface_config,
            renderer,
            scene_target,
            scene_target_view,
            blitter,
            cushion_gpu,
        })
    }

    /// Resize the surface (call on window resize).
    pub fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        self.scene_target =
            create_scene_target(&self.device, self.surface_config.width, self.surface_config.height);
        self.scene_target_view = self
            .scene_target
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.cushion_gpu.resize_target(
            &self.device,
            &mut self.renderer,
            self.surface_config.width,
            self.surface_config.height,
        );
    }

    pub fn treemap_image(&self) -> &vello::peniko::ImageData {
        self.cushion_gpu.image()
    }

    pub fn update_cushion_treemap(
        &mut self,
        layout_rects: &[LayoutRect],
        tree: &FileTree,
        config: &CushionConfig,
        color_settings: &ColorSettings,
        exclusion_rect: [f32; 4],
    ) {
        self.cushion_gpu
            .update_and_render(
                &self.device,
                &self.queue,
                layout_rects,
                tree,
                config,
                color_settings,
                exclusion_rect,
            );
    }

    /// Render a scene to the surface.
    pub fn render(&mut self, scene: &Scene) -> Result<()> {
        let surface_texture = self.surface.get_current_texture()?;

        let render_params = RenderParams {
            base_color: vello::peniko::Color::BLACK,
            width: self.surface_config.width,
            height: self.surface_config.height,
            antialiasing_method: AaConfig::Msaa16,
        };

        // Render vector scene into an intermediate storage-bindable texture.
        self.renderer.render_to_texture(
            &*self.device,
            &*self.queue,
            scene,
            &self.scene_target_view,
            &render_params,
        )?;

        // Blit into the swapchain texture for presentation.
        let surface_view = surface_texture
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present blit encoder"),
            });
        self.blitter
            .copy(&self.device, &mut encoder, &self.scene_target_view, &surface_view);
        self.queue.submit(Some(encoder.finish()));

        surface_texture.present();
        Ok(())
    }
}

fn create_scene_target(device: &wgpu::Device, width: u32, height: u32) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("scene offscreen target"),
        size: wgpu::Extent3d {
            width: width.max(1),
            height: height.max(1),
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::STORAGE_BINDING
            | wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    })
}
