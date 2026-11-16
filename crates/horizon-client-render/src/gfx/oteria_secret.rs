//! Super Secret Settings (the Super Secret Settings tile and the button of Options, off by
//! default): one of 1.8's fun screen effects over the world, as fooxy's mod brings it back.
//! After the post effects and the motion blur, before the tone curve (`docs/hdr-passes.md`,
//! step 3): the frame is copied into a texture of its own format, then drawn again through the
//! effect. Alpha is left alone; nothing is kept from one frame to the next.

pub(super) struct Secret {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    params: wgpu::Buffer,
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    copy: Option<Copy>,
    /// The effect (`pvp::SECRET_EFFECTS`), 0 off.
    pub effect: u32,
    started: std::time::Instant,
}

struct Copy {
    size: [u32; 2],
    format: wgpu::TextureFormat,
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
}

impl Secret {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Super Secret Settings"),
            source: wgpu::ShaderSource::Wgsl(include_str!("oteria_secret.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Super Secret Settings bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Super Secret Settings"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Super Secret Settings params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            layout,
            pipeline_layout,
            shader,
            params,
            pipeline: None,
            copy: None,
            effect: 0,
            started: std::time::Instant::now(),
        }
    }

    /// The effect over `target` (the frame, `size`); nothing while it is off or the frame can't
    /// be copied.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn apply(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::Texture,
        view: &wgpu::TextureView,
        size: [u32; 2],
        mut timing: Option<&mut super::pack_timing::PassTiming>,
    ) {
        let usable = target.usage().contains(wgpu::TextureUsages::COPY_SRC)
            && target.sample_count() == 1
            && size[0] > 0
            && size[1] > 0;
        if self.effect == 0 || !usable {
            self.copy = None;
            return;
        }
        let format = target.format();
        if !self.copy.as_ref().is_some_and(|c| c.size == size && c.format == format) {
            self.copy = Some(self.make_copy(device, size, format));
        }
        if self.pipeline.as_ref().is_none_or(|(f, _)| *f != format) {
            self.pipeline = Some((format, self.make_pipeline(device, format)));
        }
        let copy = self.copy.as_ref().expect("made above");
        let (_, pipeline) = self.pipeline.as_ref().expect("made above");
        let mut params = [0u8; 16];
        params[0..4].copy_from_slice(&self.effect.to_le_bytes());
        params[4..8].copy_from_slice(&(self.started.elapsed().as_secs_f32() % 3600.0).to_le_bytes());
        params[8..12].copy_from_slice(&(size[0] as f32).to_le_bytes());
        params[12..16].copy_from_slice(&(size[1] as f32).to_le_bytes());
        queue.write_buffer(&self.params, 0, &params);
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_texture(target.as_image_copy(), copy.texture.as_image_copy(), extent);
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
            label: Some("Super Secret Settings"),
            timestamp_writes: timing.as_mut().and_then(|t| t.writes("Super Secret Settings")),
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &copy.group, &[]);
        pass.draw(0..3, 0..1);
    }

    fn make_copy(&self, device: &wgpu::Device, size: [u32; 2], format: wgpu::TextureFormat) -> Copy {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Super Secret Settings frame"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Super Secret Settings frame"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: self.params.as_entire_binding(),
                },
            ],
        });
        Copy { size, format, texture, group }
    }

    fn make_pipeline(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Super Secret Settings"),
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
                entry_point: Some("fs_secret"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    // The frame's alpha (HDR's marks) stays.
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
    #[test]
    fn the_shader_validates() {
        let module = naga::front::wgsl::parse_str(include_str!("oteria_secret.wgsl")).unwrap();
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .unwrap();
    }
}
