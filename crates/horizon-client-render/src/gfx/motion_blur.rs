//! Oteria's motion blur (the Motion Blur tile, off by default): each frame of the world keeps a
//! share of the frames before it, as Lunar's and Feather's do. After the post effects and before
//! the tone curve (`docs/hdr-passes.md`, step 3): the history is the frame's own format, so the
//! light above white stays; alpha is left alone, and the history goes on a resize, a format
//! change, a title screen or the blur turned off.
//!
//! Each frame: the history drawn over the frame at `amount` (a blend constant), then the
//! frame copied into the history. The share is per frame; the game gives it for its frame rate
//! (`blur_share`), so the trail lasts as long at 60 and at 240 frames a second.

pub(super) struct MotionBlur {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    sampler: wgpu::Sampler,
    /// The pipeline for the frame's format.
    pipeline: Option<(wgpu::TextureFormat, wgpu::RenderPipeline)>,
    history: Option<History>,
    /// The share of the history in this frame, 0 (off) to below 1.
    pub amount: f32,
}

struct History {
    size: [u32; 2],
    format: wgpu::TextureFormat,
    texture: wgpu::Texture,
    group: wgpu::BindGroup,
}

impl MotionBlur {
    pub(super) fn new(device: &wgpu::Device) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Motion blur"),
            source: wgpu::ShaderSource::Wgsl(include_str!("motion_blur.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Motion blur bindings"),
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Motion blur"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Motion blur history"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Self {
            layout,
            pipeline_layout,
            shader,
            sampler,
            pipeline: None,
            history: None,
            amount: 0.0,
        }
    }

    /// The blur over `target` (the frame, `size`), then its history taken from it. Nothing,
    /// and the history dropped, while it is off or the frame can't be copied.
    pub(super) fn apply(
        &mut self,
        device: &wgpu::Device,
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
        if self.amount <= 0.0 || !usable {
            self.history = None;
            return;
        }
        let format = target.format();
        let fresh = !self
            .history
            .as_ref()
            .is_some_and(|h| h.size == size && h.format == format);
        if fresh {
            self.history = Some(self.make_history(device, size, format));
        }
        let history = self.history.as_ref().expect("made above");
        // A new history starts as this frame: nothing to lay over it yet.
        if !fresh {
            if self.pipeline.as_ref().is_none_or(|(f, _)| *f != format) {
                self.pipeline = Some((format, self.make_pipeline(device, format)));
            }
            let (_, pipeline) = self.pipeline.as_ref().expect("made above");
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
                label: Some("Motion blur"),
                timestamp_writes: timing.as_mut().and_then(|t| t.writes("Motion blur")),
                color_attachments: &attachments,
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &history.group, &[]);
            let k = f64::from(self.amount.clamp(0.0, 0.98));
            pass.set_blend_constant(wgpu::Color { r: k, g: k, b: k, a: k });
            pass.draw(0..3, 0..1);
        }
        let extent = wgpu::Extent3d {
            width: size[0],
            height: size[1],
            depth_or_array_layers: 1,
        };
        encoder.copy_texture_to_texture(
            target.as_image_copy(),
            history.texture.as_image_copy(),
            extent,
        );
    }

    fn make_history(&self, device: &wgpu::Device, size: [u32; 2], format: wgpu::TextureFormat) -> History {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Motion blur history"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            // The frame's own format, so it copies straight in.
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Motion blur history"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });
        History { size, format, texture, group }
    }

    fn make_pipeline(&self, device: &wgpu::Device, format: wgpu::TextureFormat) -> wgpu::RenderPipeline {
        // frame = history * k + frame * (1 - k), on the colour; the frame's alpha kept.
        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Constant,
                dst_factor: wgpu::BlendFactor::OneMinusConstant,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Zero,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        };
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Motion blur"),
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
                entry_point: Some("fs_blur"),
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

/// The history's share in one frame for a blur of `strength` (0..=100) at a frame `dt` seconds
/// long: what it would be at 60 frames a second, spread over the frames actually drawn.
pub fn blur_share(strength: u8, dt: f32) -> f32 {
    if strength == 0 {
        return 0.0;
    }
    // At 60 fps: 100 keeps 90% of the history each frame, 10 keeps 9%.
    let at_60 = f32::from(strength.min(100)) / 100.0 * 0.9;
    let frames = (dt * 60.0).clamp(0.05, 6.0);
    at_60.powf(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_trail_lasts_as_long_whatever_the_frame_rate() {
        assert_eq!(blur_share(0, 1.0 / 60.0), 0.0);
        let at_60 = blur_share(50, 1.0 / 60.0);
        assert!((at_60 - 0.45).abs() < 1e-4);
        // Four frames at 240 keep as much as one at 60.
        let at_240 = blur_share(50, 1.0 / 240.0);
        assert!((at_240.powi(4) - at_60).abs() < 1e-3);
        assert!(blur_share(100, 1.0 / 60.0) < 0.95);
    }
}
