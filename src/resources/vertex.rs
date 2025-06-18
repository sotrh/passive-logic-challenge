#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct TexturedVertex {
    pub position: glam::Vec2,
    pub uv: glam::Vec2,
}

impl TexturedVertex {
    pub const VB_DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<TexturedVertex>() as _,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x2,
            1 => Float32x2,
        ],
    };
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct NormalMappedVertex {
    pub position: glam::Vec3,
    pub uv: glam::Vec2,
    pub normal: glam::Vec3,
    pub tangent: glam::Vec3,
    pub bitangent: glam::Vec3,
}

impl NormalMappedVertex {
    pub const VB_DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<NormalMappedVertex>() as _,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &wgpu::vertex_attr_array![
            0 => Float32x3,
            1 => Float32x2,
            2 => Float32x3,
            3 => Float32x3,
            4 => Float32x3,
        ],
    };
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct InstanceVertex {
    pub model_matrix: glam::Mat4,
    pub normal_matrix_0: glam::Vec4,
    pub normal_matrix_1: glam::Vec4,
    pub normal_matrix_2: glam::Vec4,
}

impl InstanceVertex {
    pub const VB_DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4,
            10 => Float32x4,
            11 => Float32x4,
        ],
    };
}

impl Default for InstanceVertex {
    fn default() -> Self {
        let model_matrix = glam::Mat4::default();
        Self {
            model_matrix: Default::default(),
            normal_matrix_0: model_matrix.x_axis.xyz0(),
            normal_matrix_1: model_matrix.x_axis.xyz0(),
            normal_matrix_2: model_matrix.x_axis.xyz0(),
        }
    }
}

trait MoreSwizzles {
    fn xyz0(&self) -> glam::Vec4;
}

impl MoreSwizzles for glam::Vec3 {
    fn xyz0(&self) -> glam::Vec4 {
        glam::vec4(self.x, self.y, self.z, 0.0)
    }
}

impl MoreSwizzles for glam::Vec4 {
    fn xyz0(&self) -> glam::Vec4 {
        glam::vec4(self.x, self.y, self.z, 0.0)
    }
}