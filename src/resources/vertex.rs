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

    pub(crate) fn with_position_scale(position: glam::Vec3, scale: f32) -> Self {
        Self {
            model_matrix: glam::Mat4::from_scale_rotation_translation(
                glam::vec3(scale, scale, scale),
                Default::default(),
                position,
            ),
            normal_matrix_0: glam::vec4(1.0, 0.0, 0.0, 0.0),
            normal_matrix_1: glam::vec4(0.0, 1.0, 0.0, 0.0),
            normal_matrix_2: glam::vec4(0.0, 0.0, 1.0, 0.0),
        }
    }

    pub(crate) fn extend_between(a: glam::Vec3, b: glam::Vec3, radial_scale: f32) -> Self {
        let ab = a - b;
        let dist = ab.length();
        let dir = if dist == 0.0 {
            glam::Vec3::Y
        } else {
            ab / dist
        };
        let scale = glam::vec3(radial_scale, dist * 0.5, radial_scale);
        let position = (a + b) * 2.0;
        let rotation = glam::Quat::from_rotation_arc(glam::Vec3::Y, dir);
        let rot_mat = glam::Mat4::from_rotation_translation(rotation, glam::Vec3::ZERO);
        Self {
            model_matrix: glam::Mat4::from_scale_rotation_translation(scale, rotation, position),
            normal_matrix_0: rot_mat.x_axis,
            normal_matrix_1: rot_mat.y_axis,
            normal_matrix_2: rot_mat.z_axis,
        }
    }
}

impl Default for InstanceVertex {
    fn default() -> Self {
        let model_matrix = glam::Mat4::default();
        Self {
            model_matrix: Default::default(),
            normal_matrix_0: model_matrix.x_axis.xyz0(),
            normal_matrix_1: model_matrix.y_axis.xyz0(),
            normal_matrix_2: model_matrix.z_axis.xyz0(),
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ColoredInstance {
    color: glam::Vec4,
    model_matrix: glam::Mat4,
}

impl ColoredInstance {
    pub const VB_DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Self>() as _,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &wgpu::vertex_attr_array![
            5 => Float32x4,
            6 => Float32x4,
            7 => Float32x4,
            8 => Float32x4,
            9 => Float32x4,
        ],
    };
    
    pub(crate) fn with_position_scale(color: glam::Vec3, position: glam::Vec3, scale: f32) -> Self {
        Self {
            color: glam::vec4(color.x, color.y, color.z, 1.0),
            model_matrix: glam::Mat4::from_scale_rotation_translation(
                glam::vec3(scale, scale, scale),
                Default::default(),
                position,
            ),
        }
    }

    pub(crate) fn extend_between(
        color: glam::Vec3,
        a: glam::Vec3,
        b: glam::Vec3,
        radial_scale: f32,
    ) -> Self {
        let ab = a - b;
        let dist = ab.length();
        let dir = if dist == 0.0 {
            glam::Vec3::Y
        } else {
            ab / dist
        };
        let scale = glam::vec3(radial_scale, dist * 0.5, radial_scale);
        let position = (a + b) * 0.5;
        let rotation = glam::Quat::from_rotation_arc(glam::Vec3::Y, dir);
        log::debug!("{a:?} {b:?}, {position:?}");
        Self {
            color: glam::vec4(color.x, color.y, color.z, 1.0),
            model_matrix: glam::Mat4::from_scale_rotation_translation(scale, rotation, position),
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
