use glam::{Quat, Vec3, uvec2};
use std::io::Cursor;
use web_sys::HtmlCanvasElement;
use wgpu::{ExperimentalFeatures, SurfaceConfiguration, SurfaceTarget};
use wgpu_3dgs_viewer as gs;

//renderer state
pub struct GsRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: SurfaceConfiguration,

    pub viewer: Option<gs::Viewer>,
    pub camera: Option<gs::Camera>,

    needs_update: bool,
    last_pos: glam::Vec3,
    last_quat: glam::Quat,

    //vr support deprecated
    left_view: glam::Mat4,
    left_proj: glam::Mat4,
    right_view: glam::Mat4,
    right_proj: glam::Mat4,

    //vr support deprecated
    stereo_texture: Option<wgpu::Texture>,
}

impl GsRenderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let width = canvas.client_width() as u32;
        let height = canvas.client_height() as u32;
        if width > 0 {
            canvas.set_width(width);
        }
        if height > 0 {
            canvas.set_height(height);
        }

        let instance = wgpu::Instance::default();

        let surface = instance
            .create_surface(SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|e| format!("Error Surface Creation: {}", e))?;

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|e| format!("Error Wgpu Adapter: {:?}", e))?;

        let mut limits = adapter.limits();

        //computer shader testing
        // limits.max_compute_workgroup_storage_size = 16384;
        // limits.max_compute_invocations_per_workgroup = 256;

        //logical device high performance config
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("3DGS Device"),
                required_features: wgpu::Features::empty(),
                required_limits: limits,
                experimental_features: ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            })
            .await
            .map_err(|e| format!("Error Device Creation: {}", e))?;

        let width = canvas.width().max(1);
        let height = canvas.height().max(1);

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| {
                matches!(
                    f,
                    wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Rgba8Unorm
                )
            })
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let alpha_mode = surface_caps
            .alpha_modes
            .iter()
            .find(|m| **m == wgpu::CompositeAlphaMode::Opaque)
            .copied()
            .unwrap_or(surface_caps.alpha_modes[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_DST,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode,
            view_formats: vec![surface_format.remove_srgb_suffix()],
            desired_maximum_frame_latency: 2,
        };

        surface.configure(&device, &config);

        Ok(Self {
            surface,
            device,
            queue,
            config,
            viewer: None,
            camera: None,
            needs_update: true,
            last_pos: glam::Vec3::new(f32::MAX, f32::MAX, f32::MAX),
            last_quat: glam::Quat::IDENTITY,
            left_view: glam::Mat4::IDENTITY,
            left_proj: glam::Mat4::IDENTITY,
            right_view: glam::Mat4::IDENTITY,
            right_proj: glam::Mat4::IDENTITY,
            stereo_texture: None,
        })
    }

    pub fn resize(&mut self, new_width: u32, new_height: u32) {
        if new_width > 0 && new_height > 0 {
            self.config.width = new_width;
            self.config.height = new_height;
            self.surface.configure(&self.device, &self.config);
            self.needs_update = true;
        }
    }

    //ply parsing
    pub fn load_model(&mut self, ply_data: &[u8]) -> Result<(), String> {
        let mut cursor = Cursor::new(ply_data);

        let gaussians = gs::core::Gaussians::read_from(&mut cursor, gs::core::GaussiansSource::Ply)
            .map_err(|e| format!("Error PLY Parsing: {:?}", e))?;

        let camera = gs::Camera::new(0.1..1e4, 60f32.to_radians());

        let mut viewer = gs::Viewer::new(&self.device, self.config.view_formats[0], &gaussians)
            .map_err(|e| format!("Error View Creation: {:?}", e))?;

        viewer.update_model_transform(
            &self.queue,
            Vec3::ZERO,
            Quat::from_axis_angle(Vec3::Z, 180f32.to_radians()),
            Vec3::ONE,
        );

        viewer.update_gaussian_transform(
            &self.queue,
            1.0,
            gs::core::GaussianDisplayMode::Splat,
            gs::core::GaussianShDegree::new(0).unwrap(),
            false,
            gs::core::GaussianMaxStdDev::new(3.0).unwrap(),
        );

        viewer.update_camera(
            &self.queue,
            &camera,
            uvec2(self.config.width, self.config.height),
        );

        self.viewer = Some(viewer);
        self.camera = Some(camera);
        self.needs_update = true;

        Ok(())
    }

    //render loop
    pub fn render(&mut self) -> Result<(), String> {
        // self.needs_update = true;
        if !self.needs_update {
            return Ok(());
        }

        let (viewer, camera) = match (self.viewer.as_mut(), self.camera.as_ref()) {
            (Some(v), Some(c)) => (v, c),
            _ => return Ok(()),
        };

        viewer.update_camera(
            &self.queue,
            camera,
            uvec2(self.config.width, self.config.height),
        );

        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| format!("Error Get Texture: {:?}", e))?;

        let view = frame.texture.create_view(&wgpu::TextureViewDescriptor {
            label: Some("Render View"),
            format: Some(self.config.view_formats[0]),
            ..Default::default()
        });

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let _clear_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Clear Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 0.5,
                            b: 0.5,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        viewer.render(&mut encoder, &view, None);

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        self.needs_update = false;

        Ok(())
    }

    pub fn move_camera(&mut self, forward: f32, right: f32, up: f32, pitch: f32, yaw: f32) {
        if let Some(camera) = &mut self.camera {
            camera.move_by(forward, right);
            camera.move_up(up);
            camera.pitch_by(pitch);
            camera.yaw_by(yaw);

            self.needs_update = true;
        }
    }

    //only update flag when moved
    pub fn set_camera_with_threshold(
        &mut self,
        px: f32,
        py: f32,
        pz: f32,
        qx: f32,
        qy: f32,
        qz: f32,
        qw: f32,
    ) {
        let new_pos = glam::Vec3::new(px, py, pz);
        let new_quat = glam::Quat::from_xyzw(qx, qy, qz, qw);

        let pos_diff = self.last_pos.distance_squared(new_pos);
        let quat_diff = self.last_quat.dot(new_quat).abs();

        const POS_TRHEHSHOLD: f32 = 0.001 * 0.001;
        const ROT_THRESHOLD: f32 = 0.999999;
        if pos_diff > POS_TRHEHSHOLD || quat_diff < ROT_THRESHOLD {
            if let Some(camera) = &mut self.camera {
                camera.pos = new_pos;

                let forward = new_quat * glam::Vec3::new(0.0, 0.0, -1.0);
                camera.pitch = forward.y.asin();
                camera.yaw = forward.x.atan2(forward.z);

                self.last_pos = new_pos;
                self.last_quat = new_quat;
                self.needs_update = true;
            }
        }
    }

    //update without threshold
    pub fn set_camera(&mut self, px: f32, py: f32, pz: f32, qx: f32, qy: f32, qz: f32, qw: f32) {
        let new_pos = glam::Vec3::new(px, py, pz);
        let new_quat = glam::Quat::from_xyzw(qx, qy, qz, qw);

        if let Some(camera) = &mut self.camera {
            camera.pos = new_pos;

            let forward = new_quat * glam::Vec3::new(0.0, 0.0, -1.0);
            camera.pitch = forward.y.asin();
            camera.yaw = forward.x.atan2(forward.z);

            self.last_pos = new_pos;
            self.last_quat = new_quat;
            self.needs_update = true;
        }
    }

    pub fn set_model_transform(
        &mut self,
        px: f32,
        py: f32,
        pz: f32,
        qx: f32,
        qy: f32,
        qz: f32,
        qw: f32,
        scale: f32,
    ) {
        if let Some(viewer) = &mut self.viewer {
            viewer.update_model_transform(
                &self.queue,
                glam::Vec3::new(px, py, pz),
                glam::Quat::from_xyzw(qx, qy, qz, qw),
                glam::Vec3::splat(scale),
            );
            self.needs_update = true;
        }
    }

    //vr support deprecated
    pub fn set_stereo_cameras(
        &mut self,
        left_view: &[f32; 16],
        left_proj: &[f32; 16],
        right_view: &[f32; 16],
        right_proj: &[f32; 16],
    ) {
        self.left_view = glam::Mat4::from_cols_array(left_view);
        self.left_proj = glam::Mat4::from_cols_array(left_proj);
        self.right_view = glam::Mat4::from_cols_array(right_view);
        self.right_proj = glam::Mat4::from_cols_array(right_proj);
        self.needs_update = true;
    }

    //vr support deprecated
    //optimised with one sort, command buffer, texture for both eyes 
    pub fn render_stereo(&mut self) -> Result<(), String> {
        if !self.needs_update {
            return Ok(());
        }

        let viewer = match self.viewer.as_mut() {
            Some(v) => v,
            None => return Ok(()),
        };

        let frame = self
            .surface
            .get_current_texture()
            .map_err(|e| format!("Error Get Texture: {:?}", e))?;

        let half_width = self.config.width / 2;
        let height = self.config.height;
        let tex_size = uvec2(half_width, height);

        if self.stereo_texture.is_none()
            || self.stereo_texture.as_ref().unwrap().width() != half_width
            || self.stereo_texture.as_ref().unwrap().height() != height
        {
            self.stereo_texture = Some(self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("Stereo Offscreen Texture"),
                size: wgpu::Extent3d {
                    width: half_width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: self.config.view_formats[0],
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }));
        }

        let offscreen_tex = self.stereo_texture.as_ref().unwrap();
        let offscreen_view = offscreen_tex.create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Stereo Encoder"),
            });

        let sort_camera = gs::RawCamera {
            view_matrix: self.left_view,
            proj_matrix: self.left_proj,
        };

        viewer.update_camera(&self.queue, &sort_camera, tex_size);

        //only one sort for both eyes (left cam as ref)
        viewer.sort(&mut encoder);

        let left_camera = gs::RawCamera {
            view_matrix: self.left_view,
            proj_matrix: self.left_proj,
        };
        viewer.update_camera(&self.queue, &left_camera, tex_size);

        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Left Eye Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &offscreen_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.5,
                            g: 0.5,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        //draw for left eye
        viewer.draw(&mut encoder, &offscreen_view, None);

        //draw in right half of surface
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: offscreen_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d { x: 0, y: 0, z: 0 },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                //half width for left eye
                width: half_width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let right_camera = gs::RawCamera {
            view_matrix: self.right_view,
            proj_matrix: self.right_proj,
        };
        viewer.update_camera(&self.queue, &right_camera, tex_size);

        {
            let _clear = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Right Eye Clear"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &offscreen_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.5,
                            g: 0.5,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }

        //draw for right eye
        viewer.draw(&mut encoder, &offscreen_view, None);

        //draw in right half of surface
        encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: offscreen_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: &frame.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: half_width,
                    y: 0,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                //right half for right eye
                width: half_width,
                height,
                depth_or_array_layers: 1,
            },
        );

        self.queue.submit(std::iter::once(encoder.finish()));
        frame.present();

        self.needs_update = false;
        Ok(())
    }
}
