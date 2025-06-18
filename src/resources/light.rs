use crate::resources::buffer::BackedBuffer;

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LightUniform {
    pub position: glam::Vec4,
    pub color: glam::Vec4,
}

pub struct LightBinder {
    layout: wgpu::BindGroupLayout,
}

impl LightBinder {
    pub fn new(device: &wgpu::Device) -> Self {
        Self {
            layout: device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("LightBinder::layout"),
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
            }),
        }
    }

    pub fn bind(&self, device: &wgpu::Device, buffer: &BackedBuffer<LightUniform>) -> LightBinding {
        assert_ne!(
            buffer.buffer().usage() & wgpu::BufferUsages::UNIFORM,
            wgpu::BufferUsages::empty()
        );
        LightBinding {
            bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("LightBinding"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.buffer().as_entire_binding(),
                }],
            }),
        }
    }
    
    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}

pub struct LightBinding {
    bind_group: wgpu::BindGroup,
}

impl LightBinding {
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
