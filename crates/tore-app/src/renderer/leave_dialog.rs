//! The shared leave panel is drawn last, over menus, flight and live Watch.
use super::Renderer;

pub(super) struct Overlay {
    texture: wgpu::Texture,
    pub bind: wgpu::BindGroup,
    pub text: crate::ui_text_renderer::UiTextRenderer,
}

impl Overlay {
    fn new(renderer: &Renderer) -> Self {
        let size = wgpu::Extent3d {
            width: 640,
            height: 480,
            depth_or_array_layers: 1,
        };
        let texture = renderer.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Leave confirmation"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let sampler = renderer.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        // A confirmation must remain readable during a flight blackout.
        let veil = renderer.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Unshaded leave panel"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let bind = renderer
            .device
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Leave confirmation"),
                layout: &renderer.pipeline.get_bind_group_layout(0),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: veil.as_entire_binding(),
                    },
                ],
            });
        Self {
            texture,
            bind,
            text: crate::ui_text_renderer::UiTextRenderer::new(
                &renderer.device,
                &renderer.queue,
                renderer.config.format,
            ),
        }
    }
}

impl Renderer {
    pub(crate) fn set_leave_dialog(
        &mut self,
        image: Option<(&[u8], Option<&crate::ui_text::Layer>)>,
    ) {
        let Some((pixels, layer)) = image else {
            self.leave_dialog = None;
            return;
        };
        if self.leave_dialog.is_none() {
            self.leave_dialog = Some(Overlay::new(self));
        }
        let dialog = self.leave_dialog.as_mut().expect("created above");
        self.queue.write_texture(
            dialog.texture.as_image_copy(),
            pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(640 * 4),
                rows_per_image: Some(480),
            },
            wgpu::Extent3d {
                width: 640,
                height: 480,
                depth_or_array_layers: 1,
            },
        );
        let quads = layer.map_or_else(Vec::new, |layer| layer.quads(dialog.text.linear()));
        dialog.text.set(&self.device, &self.queue, &quads);
    }

    pub(super) fn draw_leave_dialog(&self, pass: &mut wgpu::RenderPass<'_>, size: [u32; 2]) {
        if let Some(dialog) = &self.leave_dialog {
            let v = super::Viewport::new(size[0], size[1]);
            pass.set_viewport(v.x, v.y, v.width, v.height, 0., 1.);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &dialog.bind, &[]);
            pass.draw(0..3, 0..1);
            dialog.text.draw(pass);
        }
    }
}
