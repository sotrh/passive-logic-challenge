use core::f32;
use std::sync::Arc;

use anyhow::Context;
use winit::{event::MouseButton, event_loop::ActiveEventLoop, keyboard::KeyCode, window::Window};

use crate::{
    resources::{
        self,
        buffer::{self, BackedBuffer},
        camera::{CameraBinder, CameraController, OrthoCamera, PerspectiveCamera},
        font::{Font, TextPipeline},
        light::{LightBinder, LightUniform},
        model::{MaterialBinder, ModelPipeline},
        texture::TextureBinder,
        vertex::{ColoredInstance, InstanceVertex},
        FsResources,
    }, simulation::{visualization::VisualizationPipeline, Environment, Simulation}, utils::RenderPipelineBuilder
};

pub struct Canvas {
    surface: wgpu::Surface<'static>,
    config: wgpu::SurfaceConfiguration,
    device: wgpu::Device,
    queue: wgpu::Queue,
    fullscreen_quad: wgpu::RenderPipeline,
    #[allow(unused)]
    window: Arc<Window>,
    ortho_camera: OrthoCamera,
    ortho_camera_binding: resources::camera::CameraBinding,
    font: Font,
    text_pipeline: TextPipeline,
    mspt_text: resources::font::TextBuffer,
    frame_timer: web_time::Instant,
    num_ticks: u32,
    depth_texture: wgpu::Texture,
    model_pipeline: ModelPipeline,
    visualization_pipeline: VisualizationPipeline,
    perspective_camera: PerspectiveCamera,
    perspective_camera_binding: resources::camera::CameraBinding,
    camera_controller: CameraController,
    light_buffer: BackedBuffer<LightUniform>,
    light_binding: resources::light::LightBinding,
    lmb_down: bool,
    gameplay_timer: web_time::Instant,
    simulation: Simulation,
    environment: Environment,
    solar_panel: usize,
    extractor: usize,
    node_model: resources::model::ModelId,
    connection_model: resources::model::ModelId,
    node_instances: buffer::BackedBuffer<ColoredInstance>,
    connection_instances: BackedBuffer<ColoredInstance>,
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
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
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
            .request_device(&wgpu::DeviceDescriptor {
                required_limits: wgpu::Limits::downlevel_defaults(),
                ..Default::default()
            })
            .await;
        log::info!("Requesting device");
        #[cfg(not(target_arch = "wasm32"))]
        let (device, queue) = device_request?;
        #[cfg(target_arch = "wasm32")]
        let (device, queue) = device_request.unwrap_throw();

        let mut config = surface
            .get_default_config(
                &adapter,
                window.inner_size().width.max(1),
                window.inner_size().height.max(1),
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

        let ortho_camera = OrthoCamera::new(
            0.0,
            window.inner_size().width as f32,
            window.inner_size().height as f32,
            0.0,
        );
        let camera_binder = CameraBinder::new(&device);
        let ortho_camera_binding = camera_binder.bind(&device, &ortho_camera);

        let texture_binder = TextureBinder::new(&device);

        let res = FsResources::new("res");

        let font = Font::load(&res, "fonts/OpenSans MSDF.zip", '�', &device, &queue)?;

        let text_pipeline = TextPipeline::new(
            &font,
            &camera_binder,
            config.view_formats[0],
            &texture_binder,
            &shader,
            &device,
        )?;

        let mspt_text = text_pipeline.buffer_text(&font, &device, "Tick Rate: ----")?;

        let depth_format = wgpu::TextureFormat::Depth32Float;
        let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("depth_texture"),
            size: wgpu::Extent3d {
                width: config.width,
                height: config.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: depth_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });

        let light_buffer = BackedBuffer::with_data(
            &device,
            vec![LightUniform {
                position: glam::vec4(2.0, 2.0, 2.0, 1.0),
                color: glam::vec4(1.0, 1.0, 1.0, 1.0),
            }],
            wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        );
        let light_binder = LightBinder::new(&device);
        let light_binding = light_binder.bind(&device, &light_buffer);

        let material_binder = MaterialBinder::new(&device);
        let mut model_pipeline = ModelPipeline::new(
            &device,
            config.format,
            depth_format,
            &camera_binder,
            &material_binder,
            &light_binder,
        );

        let node_model = model_pipeline.load_obj(
            &device,
            &queue,
            &material_binder,
            &res,
            "models/spherical-cube.obj",
        )?;

        let connection_model = model_pipeline.load_obj(
            &device,
            &queue,
            &material_binder,
            &res,
            "models/connection.obj",
        )?;

        let perspective_camera = PerspectiveCamera::new(
            glam::vec3(0.0, 0.0, 3.0),
            -f32::consts::FRAC_PI_2,
            0.0,
            config.width,
            config.height,
            f32::consts::FRAC_PI_4,
            0.1,
            100.0,
        );
        let perspective_camera_binding = camera_binder.bind(&device, &perspective_camera);
        let camera_controller = CameraController::new(1.0, 1.0);

        let environment = Environment::default();
        let mut simulation = Simulation::new();

        let solar_panel = simulation.add_node(10.0, 50.0, 0.9, glam::vec3(-0.5, 0.0, 0.0));
        let extractor = simulation.add_node(10.0, 20.0, 0.9, glam::vec3(0.5, 0.0, 0.0));

        simulation.connect_node(solar_panel, extractor, 1.0);
        simulation.connect_node(extractor, solar_panel, 1.0);

        let node_instances = buffer::BackedBuffer::with_data(
            &device,
            simulation.nodes().iter().map(|node| {
                ColoredInstance::with_position_scale(glam::vec3(1.0, 0.0, 0.0), node.position, 0.1)
            }).collect(),
            wgpu::BufferUsages::VERTEX,
        );

        let connection_instances = buffer::BackedBuffer::with_data(
            &device,
            simulation.connected_nodes().map(|(flow_rate, input, output)| {
                ColoredInstance::extend_between(glam::vec3(1.0, 0.0, 0.0), input.position, output.position, 0.02 * flow_rate)
            }).collect(),
            wgpu::BufferUsages::VERTEX,
        );

        let visualization_pipeline = VisualizationPipeline::new(&device, config.format, depth_format, &camera_binder);

        let last_time = web_time::Instant::now();

        Ok(Self {
            config,
            surface,
            device,
            queue,
            window,
            depth_texture,
            fullscreen_quad,
            mspt_text,
            font,
            ortho_camera,
            ortho_camera_binding,
            text_pipeline,
            model_pipeline,
            visualization_pipeline,
            node_model,
            node_instances,
            connection_model,
            connection_instances,
            perspective_camera,
            perspective_camera_binding,
            camera_controller,
            light_buffer,
            light_binding,
            frame_timer: last_time,
            num_ticks: 0,
            lmb_down: false,
            environment,
            simulation,
            solar_panel,
            extractor,
            gameplay_timer: web_time::Instant::now(),
        })
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        self.surface.configure(&self.device, &self.config);
        self.ortho_camera
            .resize(self.config.width, self.config.height);
        self.ortho_camera_binding
            .update(&self.ortho_camera, &self.queue);
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
                    &format!("Tick Rate: {:?}", self.frame_timer.elapsed() / 100),
                    &mut self.mspt_text,
                    &self.device,
                    &self.queue,
                )
                .unwrap();
            self.frame_timer = web_time::Instant::now();
            self.num_ticks = 0;
        }
        self.num_ticks += 1;

        let dt = self.gameplay_timer.elapsed();
        self.gameplay_timer = web_time::Instant::now();

        self.camera_controller
            .update_camera(&mut self.perspective_camera, dt);
        self.perspective_camera_binding
            .update(&self.perspective_camera, &self.queue);

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            format: self.config.view_formats.get(0).copied(),
            ..Default::default()
        });
        let depth_view = self.depth_texture.create_view(&Default::default());

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
                .draw_text(&mut pass, &self.mspt_text, &self.ortho_camera_binding);
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            self.visualization_pipeline.draw(
                &mut pass,
                self.node_model,
                &self.model_pipeline,
                &self.perspective_camera_binding,
                &self.node_instances,
            );
            
            self.visualization_pipeline.draw(
                &mut pass,
                self.connection_model,
                &self.model_pipeline,
                &self.perspective_camera_binding,
                &self.connection_instances,
            );
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

    pub(crate) fn handle_mouse_move(&mut self, dx: f64, dy: f64) {
        if self.lmb_down {
            self.camera_controller.process_mouse(dx, dy);
        }
    }

    pub(crate) fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => {
                self.lmb_down = pressed;
                self.window.set_cursor_visible(!pressed);
            },
            _ => {}
        }
    }

    pub(crate) fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        self.camera_controller.process_keyboard(key, pressed);
    }
}
