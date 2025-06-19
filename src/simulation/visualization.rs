use crate::resources::{
    buffer::BackedBuffer, camera::{CameraBinder, CameraBinding}, model::{ModelId, ModelPipeline}, vertex::{ColoredInstance, NormalMappedVertex}
};

pub struct VisualizationPipeline {
    pipeline: wgpu::RenderPipeline,
}

impl VisualizationPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_binder: &CameraBinder,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("VisualizationPipeline"),
            bind_group_layouts: &[camera_binder.layout()],
            push_constant_ranges: &[],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("visualization.wgsl"));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("VisualizationPipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[NormalMappedVertex::VB_DESC, ColoredInstance::VB_DESC],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview: Default::default(),
            cache: None,
        });

        Self { pipeline }
    }

    pub(crate) fn draw(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        model: ModelId,
        model_pipeline: &ModelPipeline,
        camera: &CameraBinding,
        instances: &BackedBuffer<ColoredInstance>,
    ) {
        let model = if let Some(model) = model_pipeline.get_model(model) {
            model
        } else {
            return;
        };

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, camera.bind_group(), &[]);
        pass.set_vertex_buffer(1, instances.slice());

        for mesh in &model.meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.num_elements, 0, 0..instances.len());
        }
    }
}
