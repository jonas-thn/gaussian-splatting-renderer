use web_sys::HtmlCanvasElement;
use wgpu::{ExperimentalFeatures, SurfaceTarget};

pub struct GsRenderer {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl GsRenderer {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, String> {
        let instance = wgpu::Instance::default();

        let surface = instance
            .create_surface(SurfaceTarget::Canvas(canvas))
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

        Ok(Self {
            surface,
            device,
            queue,
        })
    }
}
