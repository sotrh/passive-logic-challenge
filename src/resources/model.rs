use anyhow::*;
use std::{
    io::{BufReader, Cursor},
    path::Path,
};
use wgpu::util::DeviceExt;

use crate::resources::{
    buffer,
    camera::{CameraBinder, CameraBinding},
    light::{LightBinder, LightBinding},
    texture,
    vertex::{InstanceVertex, NormalMappedVertex},
    Resources,
};

pub struct MaterialId(usize);

pub struct Material {
    pub name: String,
    pub diffuse_texture: texture::Texture,
    pub normal_texture: texture::Texture,
    pub binding: MaterialBinding,
}

impl Material {
    pub fn new(
        device: &wgpu::Device,
        name: &str,
        diffuse_texture: texture::Texture,
        normal_texture: texture::Texture,
        material_binder: &MaterialBinder,
    ) -> Self {
        let binding = material_binder.bind(device, &diffuse_texture, &normal_texture);

        Self {
            name: String::from(name),
            diffuse_texture,
            normal_texture,
            binding,
        }
    }
}

pub struct Mesh {
    pub name: String,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub num_elements: u32,
    pub material: usize,
}

pub struct Model {
    pub meshes: Vec<Mesh>,
    pub materials: Vec<Material>,
}

impl Model {
    pub fn load_obj<P: AsRef<Path>>(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        material_binder: &MaterialBinder,
        res: &impl Resources,
        path: P,
    ) -> Result<Self> {
        let path = path.as_ref();
        let mut reader = Cursor::new(res.load_string(path)?);

        let parent_dir = path.parent().unwrap();

        let (obj_models, obj_materials) = tobj::load_obj_buf(
            &mut reader,
            &tobj::LoadOptions {
                triangulate: true,
                single_index: true,
                ..Default::default()
            },
            |path| {
                let rel_path = parent_dir.join(path);
                let mut reader = Cursor::new(res.load_string(rel_path).unwrap());
                tobj::load_mtl_buf(&mut reader)
            },
        )?;

        // We're assuming that the texture files are stored with the obj file
        let containing_folder = path.parent().context("Directory has no parent")?;

        let mut materials = Vec::new();
        for mat in obj_materials? {
            let diffuse_texture = if let Some(path) = mat.diffuse_texture {
                texture::Texture::load(device, queue, containing_folder.join(path), false)?
            } else {
                texture::Texture::from_color(
                    device,
                    queue,
                    1,
                    1,
                    wgpu::Color::WHITE,
                    true,
                    wgpu::TextureUsages::TEXTURE_BINDING,
                )
            };

            let normal_texture = if let Some(path) = mat.normal_texture {
                texture::Texture::load(device, queue, containing_folder.join(path), true)?
            } else {
                texture::Texture::from_color(
                    device,
                    queue,
                    1,
                    1,
                    wgpu::Color {
                        r: 0.5,
                        g: 0.5,
                        b: 1.0,
                        a: 1.0,
                    },
                    false,
                    wgpu::TextureUsages::TEXTURE_BINDING,
                )
            };

            materials.push(Material::new(
                device,
                &mat.name,
                diffuse_texture,
                normal_texture,
                material_binder,
            ));
        }

        if materials.is_empty() {
            materials.push(Material::new(
                device,
                "default",
                texture::Texture::from_color(
                    device,
                    queue,
                    1,
                    1,
                    wgpu::Color::WHITE,
                    true,
                    wgpu::TextureUsages::TEXTURE_BINDING,
                ),
                texture::Texture::from_color(
                    device,
                    queue,
                    1,
                    1,
                    wgpu::Color {
                        r: 0.5,
                        g: 0.5,
                        b: 1.0,
                        a: 1.0,
                    },
                    false,
                    wgpu::TextureUsages::TEXTURE_BINDING,
                ),
                material_binder,
            ));
        }

        let mut meshes = Vec::new();
        for m in obj_models {
            let mut vertices = Vec::new();
            for i in 0..m.mesh.positions.len() / 3 {
                vertices.push(NormalMappedVertex {
                    position: glam::vec3(
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ),
                    uv: glam::vec2(m.mesh.texcoords[i * 2], m.mesh.texcoords[i * 2 + 1]),
                    normal: glam::vec3(
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ),
                    // We'll calculate these later
                    tangent: [0.0; 3].into(),
                    bitangent: [0.0; 3].into(),
                });
            }

            let indices = &m.mesh.indices;

            // Calculate tangents and bitangets. We're going to
            // use the triangles, so we need to loop through the
            // indices in chunks of 3
            for c in indices.chunks(3) {
                let v0 = vertices[c[0] as usize];
                let v1 = vertices[c[1] as usize];
                let v2 = vertices[c[2] as usize];

                let pos0 = v0.position;
                let pos1 = v1.position;
                let pos2 = v2.position;

                let uv0 = v0.uv;
                let uv1 = v1.uv;
                let uv2 = v2.uv;

                // Calculate the edges of the triangle
                let delta_pos1 = pos1 - pos0;
                let delta_pos2 = pos2 - pos0;

                // This will give us a direction to calculate the
                // tangent and bitangent
                let delta_uv1 = uv1 - uv0;
                let delta_uv2 = uv2 - uv0;

                // Solving the following system of equations will
                // give us the tangent and bitangent.
                //     delta_pos1 = delta_uv1.x * T + delta_u.y * B
                //     delta_pos2 = delta_uv2.x * T + delta_uv2.y * B
                // Luckily, the place I found this equation provided
                // the solution!
                let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
                let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
                let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

                // We'll use the same tangent/bitangent for each vertex in the triangle
                vertices[c[0] as usize].tangent = tangent.into();
                vertices[c[1] as usize].tangent = tangent.into();
                vertices[c[2] as usize].tangent = tangent.into();

                vertices[c[0] as usize].bitangent = bitangent.into();
                vertices[c[1] as usize].bitangent = bitangent.into();
                vertices[c[2] as usize].bitangent = bitangent.into();
            }

            let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Vertex Buffer", path)),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(&format!("{:?} Index Buffer", path)),
                contents: bytemuck::cast_slice(&m.mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            meshes.push(Mesh {
                name: m.name,
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            });
        }

        Ok(Self { meshes, materials })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ModelId(usize);

pub struct ModelPipeline {
    draw_model_pipeline: wgpu::RenderPipeline,
    models: Vec<Model>,
}

impl ModelPipeline {
    pub fn new(
        device: &wgpu::Device,
        surface_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        camera_binder: &CameraBinder,
        material_binder: &MaterialBinder,
        light_binder: &LightBinder,
    ) -> Self {
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ModelPipeline PipelineLayout"),
            bind_group_layouts: &[
                material_binder.layout(),
                camera_binder.layout(),
                light_binder.layout(),
            ],
            push_constant_ranges: &[],
        });

        let module = device.create_shader_module(wgpu::include_wgsl!("normal_mapped.wgsl"));

        let draw_model_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ModelPipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[NormalMappedVertex::VB_DESC, InstanceVertex::VB_DESC],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            multiview: None,
            cache: None,
        });

        Self {
            draw_model_pipeline,
            models: Vec::new(),
        }
    }

    pub fn load_obj<P: AsRef<Path>>(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        material_binder: &MaterialBinder,
        res: &impl Resources,
        path: P,
    ) -> anyhow::Result<ModelId> {
        let id = ModelId(self.models.len());

        self.models
            .push(Model::load_obj(device, queue, material_binder, res, path)?);

        Ok(id)
    }

    pub fn draw<'a, 'b: 'a>(
        &'a self,
        pass: &'a mut wgpu::RenderPass<'b>,
        model: ModelId,
        camera: &CameraBinding,
        lights: &LightBinding,
        instances: &buffer::BackedBuffer<InstanceVertex>,
    ) {
        let model = match self.models.get(model.0) {
            Some(model) => model,
            None => return,
        };

        pass.set_pipeline(&self.draw_model_pipeline);
        pass.set_bind_group(1, camera.bind_group(), &[]);
        pass.set_bind_group(2, lights.bind_group(), &[]);
        pass.set_vertex_buffer(1, instances.slice());

        for mesh in &model.meshes {
            let mat = &model.materials[mesh.material];
            pass.set_bind_group(0, mat.binding.bind_group(), &[]);
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.draw_indexed(0..mesh.num_elements, 0, 0..instances.len());
        }
    }
    
    pub(crate) fn get_model(&self, model: ModelId) -> Option<&Model> {
        self.models.get(model.0)
    }
}

pub struct MaterialBinder {
    layout: wgpu::BindGroupLayout,
}

impl MaterialBinder {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("MaterialBinder"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    count: None,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                },
            ],
        });

        Self { layout }
    }

    pub fn bind(
        &self,
        device: &wgpu::Device,
        diffuse_texture: &texture::Texture,
        normal_texture: &texture::Texture,
    ) -> MaterialBinding {
        MaterialBinding {
            bind_group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                layout: &self.layout,
                label: None,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&normal_texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&normal_texture.sampler),
                    },
                ],
            }),
        }
    }

    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}

pub struct MaterialBinding {
    bind_group: wgpu::BindGroup,
}

impl MaterialBinding {
    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
