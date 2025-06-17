use core::f32;

use wgpu::util::{BufferInitDescriptor, DeviceExt};
use winit::{dpi::PhysicalPosition, event::MouseScrollDelta, keyboard::KeyCode};

pub trait Camera {
    fn view_position(&self) -> glam::Vec4;
    fn view_proj(&self) -> glam::Mat4;
}

#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraUniform {
    pub view_pos: glam::Vec4,
    pub view_proj: glam::Mat4,
}

pub struct CameraBinder {
    layout: wgpu::BindGroupLayout,
}

impl CameraBinder {
    pub fn new(device: &wgpu::Device) -> Self {
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("CameraBinder"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        Self { layout }
    }

    pub fn bind(&self, device: &wgpu::Device, camera: &impl Camera) -> CameraBinding {
        let buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("CameraBinding::buffer"),
            contents: bytemuck::bytes_of(&CameraUniform {
                view_pos: camera.view_position(),
                view_proj: camera.view_proj(),
            }),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::UNIFORM,
        });

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("CameraBinding::bind_group"),
            layout: &self.layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: buffer.as_entire_binding(),
            }],
        });

        CameraBinding { bind_group, buffer }
    }

    pub(crate) fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }
}

pub struct CameraBinding {
    buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
}

impl CameraBinding {
    pub fn update(&mut self, camera: &impl Camera, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.buffer,
            0,
            bytemuck::bytes_of(&CameraUniform {
                view_pos: camera.view_position(),
                view_proj: camera.view_proj(),
            }),
        );
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}

#[derive(Debug)]
pub struct OrthoCamera {
    left: f32,
    right: f32,
    bottom: f32,
    top: f32,
}

impl OrthoCamera {
    pub fn new(left: f32, right: f32, bottom: f32, top: f32) -> Self {
        Self {
            left,
            right,
            bottom,
            top,
        }
    }

    pub(crate) fn resize(&mut self, width: u32, height: u32) {
        self.right = width as f32;
        self.bottom = height as f32;
    }
}

impl Camera for OrthoCamera {
    fn view_position(&self) -> glam::Vec4 {
        glam::vec4(
            (self.left + self.right) * 0.5,
            (self.bottom + self.top) * 0.5,
            0.0,
            1.0,
        )
    }
    fn view_proj(&self) -> glam::Mat4 {
        glam::Mat4::orthographic_rh(self.left, self.right, self.bottom, self.top, 0.0, 1.0)
    }
}

const SAFE_FRAC_PI_2: f32 = f32::consts::FRAC_PI_2 - 0.0001;

#[derive(Debug)]
pub struct PerspectiveCamera {
    pub position: glam::Vec3,
    yaw: f32,
    pitch: f32,
    aspect: f32,
    fovy: f32,
    znear: f32,
    zfar: f32,
}

impl PerspectiveCamera {
    pub fn new<V: Into<glam::Vec3>>(
        position: V,
        yaw: f32,
        pitch: f32,
        width: u32,
        height: u32,
        fovy: f32,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            position: position.into(),
            yaw,
            pitch,
            aspect: width as f32 / height as f32,
            fovy,
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn calc_view(&self) -> glam::Mat4 {
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();

        glam::Mat4::look_to_rh(
            self.position,
            glam::Vec3::new(cos_pitch * cos_yaw, sin_pitch, cos_pitch * sin_yaw).normalize(),
            glam::Vec3::Y,
        )
    }

    pub fn calc_proj(&self) -> glam::Mat4 {
        glam::Mat4::perspective_rh(self.fovy, self.aspect, self.znear, self.zfar)
    }
}

impl Camera for PerspectiveCamera {
    fn view_position(&self) -> glam::Vec4 {
        glam::vec4(
            self.position.x,
            self.position.y,
            self.position.z,
            1.0,
        )
    }

    fn view_proj(&self) -> glam::Mat4 {
        self.calc_proj() * self.calc_view()
    }
}

#[derive(Debug)]
pub struct CameraController {
    amount_left: f32,
    amount_right: f32,
    amount_forward: f32,
    amount_backward: f32,
    amount_up: f32,
    amount_down: f32,
    rotate_horizontal: f32,
    rotate_vertical: f32,
    scroll: f32,
    speed: f32,
    sensitivity: f32,
}

impl CameraController {
    pub fn new(speed: f32, sensitivity: f32) -> Self {
        Self {
            amount_left: 0.0,
            amount_right: 0.0,
            amount_forward: 0.0,
            amount_backward: 0.0,
            amount_up: 0.0,
            amount_down: 0.0,
            rotate_horizontal: 0.0,
            rotate_vertical: 0.0,
            scroll: 0.0,
            speed,
            sensitivity,
        }
    }

    pub fn process_keyboard(&mut self, key: KeyCode, pressed: bool, speed: f32) -> bool {
        let amount = if pressed { 1.0 } else { 0.0 } * speed;
        match key {
            KeyCode::KeyW | KeyCode::ArrowUp => {
                self.amount_forward = amount;
                true
            }
            KeyCode::KeyS | KeyCode::ArrowDown => {
                self.amount_backward = amount;
                true
            }
            KeyCode::KeyA | KeyCode::ArrowLeft => {
                self.amount_left = amount;
                true
            }
            KeyCode::KeyD | KeyCode::ArrowRight => {
                self.amount_right = amount;
                true
            }
            KeyCode::Space => {
                self.amount_up = amount;
                true
            }
            KeyCode::ShiftLeft => {
                self.amount_down = amount;
                true
            }
            _ => false,
        }
    }

    pub fn process_mouse(&mut self, mouse_dx: f64, mouse_dy: f64) {
        self.rotate_horizontal = mouse_dx as f32;
        self.rotate_vertical = mouse_dy as f32;
    }

    #[allow(unused)]
    pub fn process_mouse_scroll(&mut self, delta: &MouseScrollDelta) {
        self.scroll = match delta {
            // I'm assuming a line is about 100 pixels
            MouseScrollDelta::LineDelta(_, scroll) => -scroll * 0.5,
            MouseScrollDelta::PixelDelta(PhysicalPosition { y: scroll, .. }) => -*scroll as f32,
        };
    }

    pub fn update_camera(&mut self, camera: &mut PerspectiveCamera, dt: web_time::Duration) {
        let dt = dt.as_secs_f32();

        // Move forward/backward and left/right
        let (yaw_sin, yaw_cos) = camera.yaw.sin_cos();
        let forward = glam::Vec3::new(yaw_cos, 0.0, yaw_sin).normalize();
        let right = glam::Vec3::new(-yaw_sin, 0.0, yaw_cos).normalize();
        camera.position += forward * (self.amount_forward - self.amount_backward) * self.speed * dt;
        camera.position += right * (self.amount_right - self.amount_left) * self.speed * dt;

        // Move in/out (aka. "zoom")
        // Note: this isn't an actual zoom. The camera's position
        // changes when zooming. I've added this to make it easier
        // to get closer to an object you want to focus on.
        let (pitch_sin, pitch_cos) = camera.pitch.sin_cos();
        let scrollward =
            glam::Vec3::new(pitch_cos * yaw_cos, pitch_sin, pitch_cos * yaw_sin).normalize();
        camera.position += scrollward * self.scroll * self.speed * self.sensitivity * dt;
        self.scroll = 0.0;

        // Move up/down. Since we don't use roll, we can just
        // modify the y coordinate directly.
        camera.position.y += (self.amount_up - self.amount_down) * self.speed * dt;

        // Rotate
        camera.yaw += self.rotate_horizontal * self.sensitivity * dt;
        camera.pitch += -self.rotate_vertical * self.sensitivity * dt;

        // If process_mouse isn't called every frame, these values
        // will not get set to zero, and the camera will rotate
        // when moving in a non cardinal direction.
        self.rotate_horizontal = 0.0;
        self.rotate_vertical = 0.0;

        // Keep the camera's angle from going too high/low.
        if camera.pitch < -SAFE_FRAC_PI_2 {
            camera.pitch = -SAFE_FRAC_PI_2;
        } else if camera.pitch > SAFE_FRAC_PI_2 {
            camera.pitch = SAFE_FRAC_PI_2;
        }
    }
}
