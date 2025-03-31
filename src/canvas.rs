use std::sync::Arc;

use anyhow::Context;
use winit::{event_loop::ActiveEventLoop, window::Window};

use crate::{resources::{self, camera::{CameraBinder, OrthoCamera}, font::{Font, TextPipeline}, FsResources}, utils::RenderPipelineBuilder};

pub struct Canvas {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    fullscreen_quad: wgpu::RenderPipeline,
    #[allow(unused)]
    window: Arc<Window>,
    camera: OrthoCamera,
    camera_binding: resources::camera::CameraBinding,
    font: Font,
    text_pipeline: TextPipeline,
    mspt_text: resources::font::TextBuffer,
    last_time: std::time::Instant,
    num_ticks: u32,
}

impl Canvas {
    pub async fn new(window: Arc<Window>) -> anyhow::Result<Self> {
        #[allow(unused_mut)]
        let mut backends = wgpu::Backends::all();
        #[cfg(target_arch = "wasm32")]
        let is_webgpu_supported = wgpu::util::is_browser_webgpu_supported().await;
        #[cfg(target_arch = "wasm32")]
        if !is_webgpu_supported {
            let window = wgpu::web_sys::window().unwrap_throw();
            let document = window.document().unwrap_throw();
            let h1 = document
                .get_element_by_id("error")
                .unwrap_throw()
                .dyn_into::<wgpu::web_sys::HtmlElement>()
                .unwrap_throw();

            h1.set_class_name("revealed");

            anyhow::bail!("This example requires WebGPU");
        }
        log::info!("Backends: {backends:?}");
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        log::info!("Creating surface");
        let surface = instance.create_surface(window.clone())?;
        log::info!("Requesting adapter");
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .with_context(|| "No compatible adapter")?;
        let device_request = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_limits: wgpu::Limits::downlevel_defaults(),
                    ..Default::default()
                },
                None,
            )
            .await;
        log::info!("Requesting device");
        #[cfg(not(target_arch = "wasm32"))]
        let (device, queue) = device_request?;
        #[cfg(target_arch = "wasm32")]
        let (device, queue) = device_request.unwrap_throw();

        let mut config = surface
            .get_default_config(
                &adapter,
                window.inner_size().width,
                window.inner_size().height,
            )
            .with_context(|| "Surface is invalid")?;
        config.view_formats.push(config.format.add_srgb_suffix());

        #[cfg(not(target_arch = "wasm32"))]
        surface.configure(&device, &config);

        log::info!("Creating canvas pipeline");
        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));
        let fullscreen_quad = RenderPipelineBuilder::new()
            .vertex(wgpu::VertexState {
                module: &shader,
                entry_point: Some("fullscreen_quad"),
                compilation_options: Default::default(),
                buffers: &[],
            })
            .fragment(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("canvas"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.view_formats[0],
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            })
            .build(&device)?;

            let camera = OrthoCamera::new(
                0.0,
                window.inner_size().width as f32,
                window.inner_size().height as f32,
                0.0,
            );
            let camera_binder = CameraBinder::new(&device);
            let camera_binding = camera_binder.bind(&device, &camera);
    
            let texture_bindgroup_layout =
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("texture_bindgroup_layout"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

            let res = FsResources::new("res");
    
            let font = Font::load(&res, "OpenSans MSDF.zip", '�', &device, &queue)?;
    
            let text_pipeline = TextPipeline::new(
                &font,
                &camera_binder,
                config.view_formats[0],
                &texture_bindgroup_layout,
                &shader,
                &device,
            )?;
    
            let mspt_text = text_pipeline.buffer_text(&font, &device, "Tick Rate: ----")?;
    
            let last_time = web_time::Instant::now();
    
            Ok(Self {
                config,
                surface,
                device,
                queue,
                window,
                fullscreen_quad,
                mspt_text,
                font,
                camera,
                camera_binding,
                text_pipeline,
                last_time,
                num_ticks: 0,
            })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.camera.resize(self.config.width, self.config.height);
        self.camera_binding.update(&self.camera, &self.queue);
    }

    pub fn render(&mut self, event_loop: &ActiveEventLoop) {
        self.window.request_redraw();
        let frame = match self.surface.get_current_texture() {
            Ok(frame) => frame,
            Err(wgpu::SurfaceError::Outdated) => {
                return;
            }
            Err(e) => {
                log::error!("{e}");
                event_loop.exit();
                return;
            }
        };
        
        if self.num_ticks == 100 {
            self.text_pipeline
                .update_text(
                    &self.font,
                    &format!("Tick Rate: {:?}", self.last_time.elapsed() / 100),
                    &mut self.mspt_text,
                    &self.device,
                    &self.queue,
                )
                .unwrap();
            self.last_time = web_time::Instant::now();
            self.num_ticks = 0;
        }
        self.num_ticks += 1;

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: self.config.view_formats.get(0).copied(),
            ..Default::default()
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });

            pass.set_pipeline(&self.fullscreen_quad);
            pass.draw(0..3, 0..1);


            self.text_pipeline
                .draw_text(&mut pass, &self.mspt_text, &self.camera_binding);
        }

        self.queue.submit([encoder.finish()]);
        frame.present();
    }

    pub fn project_point(&self, x: f32, y: f32) -> glam::Vec2 {
        let aspect_ratio = self.config.width as f32 / self.config.height as f32;
        glam::vec2(
            x / self.config.width.max(1) as f32 * aspect_ratio,
            1.0 - y / self.config.height.max(1) as f32,
        )
    }
}
