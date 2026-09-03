use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;
use fluent_bundle::FluentValue;
use image::RgbaImage;
use slint::{ComponentHandle, ModelRc, VecModel, Weak};
use tbx_core::validation::{self, IssueLevel};
use tbx_entitlements::Capability;
use tbx_locale::LocaleManager;
use tbx_state::{AppEvent, AppState};
use crate::{MainWindow, Nav, PreviewApi, Warning};
const PREVIEW_SIZE: u32 = 512;
const PLANE_CAM: ([f32; 3], f32) = ([0.0, 1.5, 2.4], 0.75);
const SPHERE_CAM: ([f32; 3], f32) = ([0.0, 0.35, 2.8], 0.5);
const LIGHTS: [([f32; 3], [f32; 3], [f32; 3]); 3] = [
    ([0.55, 0.85, 0.55], [1.0, 0.98, 0.94], [0.14, 0.15, 0.18]),
    ([0.25, 0.9, 0.25], [1.0, 0.9, 0.72], [0.10, 0.11, 0.15]),
    ([-0.7, 0.35, -0.55], [0.7, 0.8, 1.0], [0.08, 0.09, 0.12]),
];
struct Material {
    albedo: Arc<RgbaImage>,
    normal: Arc<RgbaImage>,
    rough: Arc<RgbaImage>,
    ao: Arc<RgbaImage>,
}
fn gather_material(state: &AppState) -> Material {
    let neutral = |px: [u8; 4]| Arc::new(RgbaImage::from_pixel(16, 16, image::Rgba(px)));
    let p = state.project.read().unwrap_or_else(|e| e.into_inner());
    let albedo = p
        .source
        .as_ref()
        .or(p.tileable.as_ref())
        .or(p.atlas.as_ref())
        .or(p.packed.as_ref())
        .map(|s| Arc::clone(&s.image))
        .unwrap_or_else(|| neutral([120, 120, 120, 255]));
    let outs = p.maps.outputs.as_ref();
    let normal = outs
        .and_then(|o| o.normal.as_ref().map(|i| Arc::new(i.clone())))
        .unwrap_or_else(|| neutral([128, 128, 255, 255]));
    let rough = outs
        .and_then(|o| o.roughness.as_ref().map(|g| Arc::new(g.to_rgba())))
        .unwrap_or_else(|| neutral([200, 200, 200, 255]));
    let ao = outs
        .and_then(|o| o.ao.as_ref().map(|g| Arc::new(g.to_rgba())))
        .unwrap_or_else(|| neutral([255, 255, 255, 255]));
    Material { albedo, normal, rough, ao }
}
#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct PreviewUniforms {
    camera_pos: [f32; 3],
    tan_fov: f32,
    light_dir: [f32; 3],
    geometry: f32,
    light_color: [f32; 3],
    pad0: f32,
    ambient: [f32; 3],
    pad1: f32,
    zoom: f32,
    yaw: f32,
    pad2: [f32; 2],
}
#[derive(Clone, Copy)]
struct ViewState {
    zoom: f32,
    yaw: f32,
}
const PREVIEW_SHADER: &str = r#"
struct U {
    camera_pos: vec3<f32>,
    tan_fov: f32,
    light_dir: vec3<f32>,
    geometry: f32,
    light_color: vec3<f32>,
    pad0: f32,
    ambient: vec3<f32>,
    pad1: f32,
    zoom: f32,
    yaw: f32,
    pad2: vec2<f32>,
};
@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var t_albedo: texture_2d<f32>;
@group(0) @binding(3) var t_normal: texture_2d<f32>;
@group(0) @binding(4) var t_rough: texture_2d<f32>;
@group(0) @binding(5) var t_ao: texture_2d<f32>;
struct VOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) ndc: vec2<f32>,
};
@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var p = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    var o: VOut;
    o.clip = vec4(p[i], 0.0, 1.0);
    o.ndc = p[i];
    return o;
}
fn tbn(n: vec3<f32>) -> mat3x3<f32> {
    let up = select(vec3(1.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0), abs(n.y) < 0.999);
    let t = normalize(cross(up, n));
    let b = cross(n, t);
    return mat3x3<f32>(t, b, n);
}
@fragment
fn fs(v: VOut) -> @location(0) vec4<f32> {
    let bg = vec4(0.055, 0.075, 0.125, 1.0);
    let cam = u.camera_pos / max(u.zoom, 0.05);
    let fwd = normalize(-cam);
    let right = normalize(cross(fwd, vec3(0.0, 1.0, 0.0)));
    let up = cross(right, fwd);
    let rd = normalize(fwd + u.tan_fov * (v.ndc.x * right + v.ndc.y * up));
    var pos: vec3<f32>;
    var n: vec3<f32>;
    var uv: vec2<f32>;
    if (u.geometry > 0.5) {
        let b = dot(cam, rd);
        let c = dot(cam, cam) - 1.0;
        let h = b * b - c;
        if (h < 0.0) { return bg; }
        let t = -b - sqrt(h);
        if (t < 0.0) { return bg; }
        pos = cam + rd * t;
        n = normalize(pos);
        let ca = cos(u.yaw);
        let sa = sin(u.yaw);
        let rx = ca * n.x + sa * n.z;
        let rz = -sa * n.x + ca * n.z;
        uv = vec2(
            0.5 + atan2(rz, rx) / 6.28318530718,
            0.5 - asin(clamp(n.y, -1.0, 1.0)) / 3.14159265359,
        );
    } else {
        if (rd.y >= -0.001) { return bg; }
        let t = -cam.y / rd.y;
        pos = cam + rd * t;
        n = vec3(0.0, 1.0, 0.0);
        uv = pos.xz * 0.5;
    }
    let albedo = textureSample(t_albedo, samp, uv).rgb;
    let nm = textureSample(t_normal, samp, uv).rgb * 2.0 - 1.0;
    let rough = textureSample(t_rough, samp, uv).r;
    let occ = textureSample(t_ao, samp, uv).r;
    let nrm = normalize(tbn(n) * nm);
    let l = normalize(u.light_dir);
    let ndl = clamp(dot(nrm, l), 0.0, 1.0);
    let view_dir = normalize(cam - pos);
    let hvec = normalize(l + view_dir);
    let spec_pow = mix(8.0, 160.0, pow(1.0 - rough, 2.0));
    let spec = pow(clamp(dot(nrm, hvec), 0.0, 1.0), spec_pow) * (1.0 - rough) * 0.35;
    var col = albedo * (u.ambient + u.light_color * ndl) * occ;
    col += u.light_color * spec;
    col = col / (col + vec3(1.0)) * 1.15;
    return vec4(col, 1.0);
}
"#;
struct GpuPreview {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buf: wgpu::Buffer,
    out_tex: wgpu::Texture,
    out_view: wgpu::TextureView,
    readback_buf: wgpu::Buffer,
    size: u32,
}
impl GpuPreview {
    async fn new(size: u32) -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default(), None)
            .await
            .ok()?;
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(PREVIEW_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
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
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ]
            .into_iter()
            .chain((2..6).map(|binding| wgpu::BindGroupLayoutEntry {
                binding,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            }))
            .collect::<Vec<_>>(),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs",
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            ..Default::default()
        });
        let uniform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: std::mem::size_of::<PreviewUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let out_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: size, height: size, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let out_view = out_tex.create_view(&Default::default());
        let readback_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (size * size * 4) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Some(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
            sampler,
            uniform_buf,
            out_tex,
            out_view,
            readback_buf,
            size,
        })
    }
    fn texture_from_rgba(&self, img: &RgbaImage) -> wgpu::Texture {
        let (w, h) = img.dimensions();
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            img.as_raw(),
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(w * 4),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        tex
    }
    fn render(
        &mut self,
        mat: &Material,
        geometry: i32,
        lighting: i32,
        view: ViewState,
    ) -> Result<RgbaImage, &'static str> {
        let textures: [wgpu::Texture; 4] = [
            self.texture_from_rgba(&mat.albedo),
            self.texture_from_rgba(&mat.normal),
            self.texture_from_rgba(&mat.rough),
            self.texture_from_rgba(&mat.ao),
        ];
        let views: [wgpu::TextureView; 4] = textures.map(|t| t.create_view(&Default::default()));
        let (dir_raw, light_color, ambient) = LIGHTS[lighting.clamp(0, 2) as usize];
        let len = (dir_raw[0] * dir_raw[0] + dir_raw[1] * dir_raw[1] + dir_raw[2] * dir_raw[2]).sqrt();
        let light_dir = [dir_raw[0] / len, dir_raw[1] / len, dir_raw[2] / len];
        let (camera_pos, tan_fov) = if geometry == 1 { SPHERE_CAM } else { PLANE_CAM };
        let uniforms = PreviewUniforms {
            camera_pos,
            tan_fov,
            light_dir,
            geometry: geometry as f32,
            light_color,
            pad0: 0.0,
            ambient,
            pad1: 0.0,
            zoom: view.zoom,
            yaw: view.yaw,
            pad2: [0.0; 2],
        };
        self.queue.write_buffer(&self.uniform_buf, 0, bytemuck::bytes_of(&uniforms));
        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.uniform_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&views[0]),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&views[1]),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&views[2]),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::TextureView(&views[3]),
                },
            ],
        });
        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: None,
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.out_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_texture_to_buffer(
            wgpu::ImageCopyTexture {
                texture: &self.out_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::ImageCopyBuffer {
                buffer: &self.readback_buf,
                layout: wgpu::ImageDataLayout {
                    offset: 0,
                    bytes_per_row: Some(self.size * 4),
                    rows_per_image: Some(self.size),
                },
            },
            wgpu::Extent3d { width: self.size, height: self.size, depth_or_array_layers: 1 },
        );
        self.queue.submit(std::iter::once(encoder.finish()));
        let (tx, rx) = mpsc::channel();
        self.readback_buf
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |res| {
                let _ = tx.send(res);
            });
        loop {
            match rx.try_recv() {
                Ok(Ok(())) => break,
                Ok(Err(_)) => return Err("preview readback failed"),
                Err(mpsc::TryRecvError::Empty) => {
                    let _ = self.device.poll(wgpu::Maintain::Poll);
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    return Err("preview readback channel lost");
                }
            }
        }
        let pixels = {
            let view = self.readback_buf.slice(..).get_mapped_range();
            view.to_vec()
        };
        self.readback_buf.unmap();
        RgbaImage::from_raw(self.size, self.size, pixels).ok_or("preview readback size mismatch")
    }
}
fn collect_warnings(state: &AppState, locale: &LocaleManager) -> Vec<Warning> {
    let p = state.project.read().unwrap_or_else(|e| e.into_inner());
    let mut list = Vec::new();
    let Some(outs) = p.maps.outputs.as_ref() else { return list };
    let basic_only = state.gate.is_locked(Capability::PreviewFullValidationSuite);
    for issue in validation::validate_maps(outs) {
        if basic_only && issue.level != IssueLevel::Error {
            continue;
        }
        let args: Vec<(&str, FluentValue<'_>)> = issue
            .args
            .iter()
            .map(|(k, v)| (*k, v.as_str().into()))
            .collect();
        let level = match issue.level {
            IssueLevel::Info => 0,
            IssueLevel::Warning => 1,
            IssueLevel::Error => 2,
        };
        list.push(Warning { level, text: locale.tr_args(issue.key, &args).into() });
    }
    list
}
fn render_now(
    window: &MainWindow,
    state: Arc<AppState>,
    locale: Arc<LocaleManager>,
    renderer: Arc<Mutex<Option<GpuPreview>>>,
    view: Arc<Mutex<ViewState>>,
    is_rendering: Arc<std::sync::atomic::AtomicBool>,
) {
    if is_rendering.compare_exchange(
        false, true,
        std::sync::atomic::Ordering::AcqRel,
        std::sync::atomic::Ordering::Acquire,
    ).is_err() {
        return;
    }
    let api = window.global::<PreviewApi>();
    api.set_busy(true);
    api.set_status(locale.tr("common-busy").into());
    let geometry = api.get_geometry();
    let lighting = api.get_lighting_index();
    let view_state = *view.lock().unwrap_or_else(|e| e.into_inner());
    let t0 = Instant::now();
    let weak_w = window.as_weak();
    std::thread::spawn(move || {
        let mut slot = renderer.lock().unwrap_or_else(|e| e.into_inner());
        if slot.is_none() {
            *slot = pollster::block_on(GpuPreview::new(PREVIEW_SIZE));
        }
        let Some(mut ren) = slot.take() else {
            is_rendering.store(false, std::sync::atomic::Ordering::Release);
            let locale_no_gpu = locale.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = weak_w.upgrade() else { return };
                let api = window.global::<PreviewApi>();
                api.set_status(locale_no_gpu.tr("preview-status-no-gpu").into());
                api.set_busy(false);
            });
            return;
        };
        let mat = gather_material(&state);
        let rendered = ren.render(&mat, geometry, lighting, view_state);
        let secs = t0.elapsed();
        let img = match rendered {
            Ok(img) => img,
            Err(msg) => {
                if cfg!(debug_assertions) {
                    eprintln!("[texelbox] preview render failed: {msg}");
                }
                *slot = Some(ren);
                is_rendering.store(false, std::sync::atomic::Ordering::Release);
                let locale_err = locale.clone();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(window) = weak_w.upgrade() else { return };
                    let api = window.global::<PreviewApi>();
                    api.set_status(locale_err.tr("preview-status-gpu-error").into());
                    api.set_busy(false);
                });
                return;
            }
        };
        let warnings = collect_warnings(&state, &locale);
        let status = locale.tr_args(
            "preview-status-rendered",
            &[("secs", fluent_bundle::FluentValue::from(format!("{:.2}", secs.as_secs_f64())))],
        );
        let (w, h) = (img.width(), img.height());
        let img_raw = img.into_raw();
        *slot = Some(ren);
        is_rendering.store(false, std::sync::atomic::Ordering::Release);
        let _ = slint::invoke_from_event_loop(move || {
            let Some(window) = weak_w.upgrade() else { return };
            let api = window.global::<PreviewApi>();
            let buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&img_raw, w, h);
            api.set_preview(slint::Image::from_rgba8(buffer));
            api.set_has_preview(true);
            api.set_has_warnings(!warnings.is_empty());
            api.set_warnings(ModelRc::new(VecModel::from(warnings)));
            api.set_busy(false);
            api.set_status(status.into());
        });
    });
}
pub fn sync_entitlements(window: &MainWindow, state: &AppState) {
    let gate = &state.gate;
    let api = window.global::<PreviewApi>();
    api.set_lock_sphere(gate.is_locked(Capability::PreviewSphereViewport));
    api.set_lock_lighting(gate.is_locked(Capability::PreviewMultipleLightingRigs));
}
pub fn wire(window: &MainWindow, state: Arc<AppState>, locale: Arc<LocaleManager>) {
    let renderer: Arc<Mutex<Option<GpuPreview>>> = Arc::new(Mutex::new(None));
    let view: Arc<Mutex<ViewState>> = Arc::new(Mutex::new(ViewState { zoom: 1.0, yaw: 0.0 }));
    let is_rendering = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let window_weak: Weak<MainWindow> = window.as_weak().clone();
    window.global::<PreviewApi>().on_refresh({
        let state = state.clone();
        let locale = locale.clone();
        let renderer = renderer.clone();
        let view = view.clone();
        let is_rendering = is_rendering.clone();
        move || {
            let Some(window) = window_weak.upgrade() else { return };
            render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
        }
    });
    window.global::<PreviewApi>().on_select_geometry({
        let window_weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let renderer = renderer.clone();
        let view = view.clone();
        let is_rendering = is_rendering.clone();
        move |g| {
            let Some(window) = window_weak.upgrade() else { return };
            let g = if g == 1 && state.gate.is_locked(Capability::PreviewSphereViewport) { 0 } else { g };
            window.global::<PreviewApi>().set_geometry(g);
            *view.lock().unwrap_or_else(|e| e.into_inner()) = ViewState { zoom: 1.0, yaw: 0.0 };
            render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
        }
    });
    window.global::<PreviewApi>().on_select_lighting({
        let window_weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let renderer = renderer.clone();
        let view = view.clone();
        let is_rendering = is_rendering.clone();
        move |i| {
            let Some(window) = window_weak.upgrade() else { return };
            let i = if state.gate.is_locked(Capability::PreviewMultipleLightingRigs) { 0 } else { i };
            window.global::<PreviewApi>().set_lighting_index(i);
            render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
        }
    });
    window.global::<PreviewApi>().on_zoom_view({
        let window_weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let renderer = renderer.clone();
        let view = view.clone();
        let is_rendering = is_rendering.clone();
        move |delta_y| {
            let Some(window) = window_weak.upgrade() else { return };
            {
                let mut v = view.lock().unwrap_or_else(|e| e.into_inner());
                v.zoom = (v.zoom * (1.0 - delta_y * 0.0015)).clamp(0.4, 4.0);
            }
            render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
        }
    });
    window.global::<PreviewApi>().on_drag_view({
        let window_weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let renderer = renderer.clone();
        let view = view.clone();
        let is_rendering = is_rendering.clone();
        move |dx, _dy| {
            let Some(window) = window_weak.upgrade() else { return };
            {
                let mut v = view.lock().unwrap_or_else(|e| e.into_inner());
                v.yaw += dx * 0.01;
            }
            render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
        }
    });
    window.global::<Nav>().on_tab_selected({
        let window_weak = window.as_weak().clone();
        let state = state.clone();
        let locale = locale.clone();
        let renderer = renderer.clone();
        let view = view.clone();
        let is_rendering = is_rendering.clone();
        move |tab| {
            if tab != 5 {
                return;
            }
            let Some(window) = window_weak.upgrade() else { return };
            render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
        }
    });
    let rx = state.bus.subscribe();
    let window_weak = window.as_weak().clone();
    std::thread::spawn(move || {
        while let Ok(ev) = rx.recv() {
            if !matches!(ev, AppEvent::StageUpdated(_)) {
                continue;
            }
            let window_weak = window_weak.clone();
            let state = state.clone();
            let locale = locale.clone();
            let renderer = renderer.clone();
            let view = view.clone();
            let is_rendering = is_rendering.clone();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(window) = window_weak.upgrade() else { return };
                if window.global::<Nav>().get_tab() != 5 {
                    return;
                }
                render_now(&window, state.clone(), locale.clone(), renderer.clone(), view.clone(), is_rendering.clone());
            });
        }
    });
}
