//! Oteria's environment (the Environment tile, off by default): the world dressed as a dream.
//! One pass over the drawn world reads its depth and lays over it, from back to front: an
//! aurora sky with twinkling stars, sparkles and shooting stars where nothing was drawn; waves
//! of tiles running out from the player's feet over the ground, with their edges lit; a
//! coloured fog over the distance; a China hat and a halo over the player's head in third
//! person; and flakes and sparkles floating in the air, hidden behind what is nearer.
//!
//! It runs after the world (and the world post pass), before the hand and the tone curve
//! (`docs/hdr-passes.md`, step 3): colour only, the frame's alpha left alone. Under a shader
//! pack, whose own passes draw the world, it stays off.

use glam::{DVec3, Mat4};

/// What the game gives for this frame: where the player is and the tile's settings.
#[derive(Clone, Copy, Debug)]
pub struct EnvironmentFrame {
    /// The player's feet, in the world.
    pub feet: DVec3,
    /// The top of the player's head, in the world, when the hat shows (third person).
    pub head: Option<DVec3>,
    /// The render distance, in blocks (the fog's scale).
    pub view_distance: f32,
    pub settings: crate::pvp::Environment,
}

/// The uniform block (`Env` in the shader).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    inv_view_proj: [[f32; 4]; 4],
    feet: [f32; 4],
    head: [f32; 4],
    ground: [f32; 4],
    particles: [f32; 4],
    hat: [f32; 4],
    sky: [f32; 4],
    fog: [f32; 4],
    misc: [f32; 4],
    origin: [f32; 4],
}

pub(super) struct Environment {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    uniforms: wgpu::Buffer,
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    start: std::time::Instant,
    /// This frame's, from the game; `None`: off.
    pub frame: Option<EnvironmentFrame>,
}

/// A colour as 0..1 values, in gamma as the frame stores it.
fn rgb(c: crate::pvp::Colour) -> [f32; 3] {
    let v = c.argb();
    [
        ((v >> 16) & 0xFF) as f32 / 255.0,
        ((v >> 8) & 0xFF) as f32 / 255.0,
        (v & 0xFF) as f32 / 255.0,
    ]
}

fn with(c: [f32; 3], w: f32) -> [f32; 4] {
    [c[0], c[1], c[2], w]
}

impl Environment {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Oteria environment"),
            source: wgpu::ShaderSource::Wgsl(include_str!("oteria_environment.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Oteria environment bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Depth,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Oteria environment"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Oteria environment"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            layout,
            pipeline_layout,
            shader,
            uniforms,
            pipeline: None,
            start: std::time::Instant::now(),
            frame: None,
        }
    }

    /// Whether it draws this frame (and so needs the world's depth kept).
    pub(super) fn active(&self) -> bool {
        self.frame.is_some_and(|f| {
            let s = f.settings;
            s.enabled && (s.ground || s.particles || s.hat && f.head.is_some() || s.sky || s.fog)
        })
    }

    /// The pass over `view` (the frame, `size`), reading `depth` (`depth_size`), drawn with
    /// `jittered` from `camera`. Nothing while it is off or the depth can't be read.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        format: wgpu::TextureFormat,
        depth: &wgpu::TextureView,
        depth_size: [u32; 2],
        jittered: Mat4,
        camera: DVec3,
        mut timing: Option<&mut super::pack_timing::PassTiming>,
    ) {
        if !self.active() {
            return;
        }
        let Some(frame) = self.frame else {
            return;
        };
        let texture = depth.texture();
        if texture.sample_count() != 1
            || !texture.usage().contains(wgpu::TextureUsages::TEXTURE_BINDING)
            || !texture.format().is_depth_stencil_format()
            || texture.format().has_stencil_aspect()
            || depth_size[0] == 0
            || depth_size[1] == 0
        {
            return;
        }
        let s = frame.settings;
        let inverse = jittered.inverse();
        if !inverse.is_finite() {
            return;
        }
        let rel = |p: DVec3| (p - camera).as_vec3();
        let feet = rel(frame.feet);
        let head = frame.head.filter(|_| s.hat).map(rel);
        let origin = DVec3::new(
            camera.x.rem_euclid(4096.0),
            camera.y.rem_euclid(4096.0),
            camera.z.rem_euclid(4096.0),
        )
        .as_vec3();
        let amount = |v: u8| f32::from(v.min(100)) / 100.0;
        let u = Uniforms {
            inv_view_proj: inverse.to_cols_array_2d(),
            feet: [feet.x, feet.y, feet.z, if s.ground { f32::from(s.ground_radius) } else { 0.0 }],
            head: head.map_or([0.0; 4], |h| [h.x, h.y, h.z, 1.0]),
            ground: with(rgb(s.ground_colour), 0.0),
            particles: with(rgb(s.particles_colour), if s.particles { amount(s.particles_amount) } else { 0.0 }),
            hat: with(rgb(s.hat_colour), 0.0),
            sky: with(rgb(s.sky_colour), if s.sky { 1.0 } else { 0.0 }),
            fog: with(rgb(s.fog_colour), if s.fog { amount(s.fog_density) } else { 0.0 }),
            misc: [
                // Wrapped so the clock keeps its precision in a long session.
                (self.start.elapsed().as_secs_f64() % 3600.0) as f32,
                depth_size[0] as f32,
                depth_size[1] as f32,
                frame.view_distance.max(16.0),
            ],
            origin: [origin.x, origin.y, origin.z, 0.0],
        };
        queue.write_buffer(&self.uniforms, 0, bytemuck::bytes_of(&u));
        if self.pipeline.as_ref().is_none_or(|(f, _)| *f != format) {
            self.pipeline = Some((format, self.make_pipeline(device, format)));
        }
        let (_, pipeline) = self.pipeline.as_ref().expect("made above");
        let depth_only = texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Oteria environment depth"),
            aspect: wgpu::TextureAspect::DepthOnly,
            ..Default::default()
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Oteria environment"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&depth_only),
                },
            ],
        });
        let attachments = [Some(wgpu::RenderPassColorAttachment {
            view,
            resolve_target: None,
            depth_slice: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Load,
                store: wgpu::StoreOp::Store,
            },
        })];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Oteria environment"),
            timestamp_writes: timing.as_mut().and_then(|t| t.writes("Oteria environment")),
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn make_pipeline(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        // Premultiplied over the frame, on the colour; the frame's alpha kept.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Oteria environment"),
            layout: Some(&self.pipeline_layout),
            vertex: wgpu::VertexState {
                module: &self.shader,
                entry_point: Some("vs_quad"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &self.shader,
                entry_point: Some("fs_environment"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("oteria_environment.wgsl")).unwrap();
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap();
    }

    #[test]
    fn the_uniforms_match_the_shader_block() {
        // A matrix and nine vectors, as `Env` declares them.
        assert_eq!(std::mem::size_of::<Uniforms>(), 64 + 9 * 16);
    }
}
