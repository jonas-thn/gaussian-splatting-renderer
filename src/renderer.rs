use glam::{Quat, Vec3, uvec2};
use std::io::Cursor;
use web_sys::HtmlCanvasElement;
use wgpu::{ExperimentalFeatures, SurfaceConfiguration, SurfaceTarget};
use wgpu_3dgs_viewer::{self as gs, Viewer};

pub struct GsRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: SurfaceConfiguration,

    pub viewer: Option<gs::Viewer>,
    pub camera: Option<gs::Camera>,
}

impl GsRenderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
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

        let limits = adapter.limits();

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
        let surface_format = surface_caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
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
        })
    }

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
            gs::core::GaussianShDegree::new(3).unwrap(),
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

        Ok(())
    }

    pub fn render(&mut self) -> Result<(), String> {
        let viewer = match self.viewer.as_mut() {
            Some(v) => v,
            None => return Ok(()),
        };

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

        viewer.render(&mut encoder, &view);

        self.queue.submit(std::iter::once(encoder.finish()));

        frame.present();

        Ok(())
    }
}
