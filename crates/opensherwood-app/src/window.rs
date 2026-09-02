//! Interactive window: winit event loop, fixed-rate ticks, wgpu presentation of the CPU
//! framebuffer (ADR-0002). Optional JSON-RPC on stdin so the harness can drive the real window.

use std::sync::Arc;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

use anyhow::Context;
use opensherwood_core::{Button, InputEvent, Key};
use opensherwood_render::Framebuffer;
use winit::application::ApplicationHandler;
use winit::dpi::{LogicalSize, PhysicalPosition};
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

use crate::engine::Session;
use crate::rpc;

/// Simulation ticks per second in window mode.
pub const TICK_RATE: u32 = 60;

/// How the window is presented.
#[derive(Debug, Clone, Copy)]
pub struct Presentation {
    /// Integer scale of the logical viewport when windowed.
    pub scale: u32,
    /// Do not open an audio device.
    pub mute: bool,
    /// Resizable window instead of borderless fullscreen (the default).
    pub windowed: bool,
}

/// Run the window until it is closed.
pub fn run(
    mut session: Session,
    rpc: bool,
    scenario: &str,
    presentation: Presentation,
) -> anyhow::Result<()> {
    let scenario = Session::parse_scenario(scenario).map_err(anyhow::Error::msg)?;
    session.set_audio(presentation.mute);
    session.reset(scenario, 0).map_err(anyhow::Error::msg)?;
    let (vw, vh) = session.world.as_ref().map_or((640, 480), |w| w.viewport);
    let event_loop = EventLoop::new().context("creating the event loop")?;
    event_loop.set_control_flow(ControlFlow::Poll);
    let mut app = App {
        session,
        rpc: rpc.then(rpc::spawn_stdin_reader),
        gpu: None,
        viewport: (vw, vh),
        scale: presentation.scale.max(1),
        windowed: presentation.windowed,
        pending: Vec::new(),
        last_tick: Instant::now(),
        accumulator: Duration::ZERO,
        pointer: (0.0, 0.0),
        exit: false,
    };
    event_loop.run_app(&mut app).context("event loop")?;
    Ok(())
}

struct Gpu {
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
    tex_size: (u32, u32),
}

struct App {
    session: Session,
    rpc: Option<Receiver<String>>,
    gpu: Option<Gpu>,
    viewport: (u32, u32),
    scale: u32,
    windowed: bool,
    pending: Vec<InputEvent>,
    last_tick: Instant,
    accumulator: Duration,
    pointer: (f64, f64),
    exit: bool,
}

const SHADER: &str = r"
struct VsOut { @builtin(position) pos: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VsOut {
    var out: VsOut;
    let x = f32(i32(i & 1u) * 4 - 1);
    let y = f32(i32(i >> 1u) * 4 - 1);
    out.pos = vec4<f32>(x, -y, 0.0, 1.0);
    out.uv = vec2<f32>((x + 1.0) * 0.5, (y + 1.0) * 0.5);
    return out;
}
@group(0) @binding(0) var t: texture_2d<f32>;
@group(0) @binding(1) var s: sampler;
@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    return textureSample(t, s, in.uv);
}
";

impl Gpu {
    fn new(window: Arc<Window>, tex_size: (u32, u32)) -> anyhow::Result<Self> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(window.clone())
            .context("creating surface")?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            ..Default::default()
        }))
        .context("no compatible GPU adapter")?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default(), None))
                .context("requesting device")?;
        let size = window.inner_size();
        let mut config = surface
            .get_default_config(&adapter, size.width.max(1), size.height.max(1))
            .context("surface not supported")?;
        config.present_mode = wgpu::PresentMode::AutoVsync;
        surface.configure(&device, &config);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let texture = Self::make_texture(&device, tex_size);
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("blit layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = Self::make_bind_group(&device, &layout, &texture, &sampler);
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("blit pipeline layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });
        Ok(Self {
            window,
            surface,
            device,
            queue,
            config,
            pipeline,
            texture,
            bind_group,
            tex_size,
        })
    }

    fn make_texture(device: &wgpu::Device, (w, h): (u32, u32)) -> wgpu::Texture {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("framebuffer"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        })
    }

    fn make_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
        sampler: &wgpu::Sampler,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit bind group"),
            layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    fn resize(&mut self, w: u32, h: u32) {
        self.config.width = w.max(1);
        self.config.height = h.max(1);
        self.surface.configure(&self.device, &self.config);
    }

    /// Letterboxed destination rectangle of the framebuffer inside the window (physical pixels).
    fn dest_rect(&self) -> (f32, f32, f32, f32) {
        let (ww, wh) = (self.config.width as f32, self.config.height as f32);
        let (tw, th) = (self.tex_size.0 as f32, self.tex_size.1 as f32);
        let s = (ww / tw).min(wh / th);
        let (dw, dh) = (tw * s, th * s);
        ((ww - dw) * 0.5, (wh - dh) * 0.5, dw, dh)
    }

    fn present(&mut self, frame: &Framebuffer) -> anyhow::Result<()> {
        if (frame.width, frame.height) != self.tex_size {
            self.tex_size = (frame.width, frame.height);
            self.texture = Self::make_texture(&self.device, self.tex_size);
            let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                ..Default::default()
            });
            let layout = self.pipeline.get_bind_group_layout(0);
            self.bind_group = Self::make_bind_group(&self.device, &layout, &self.texture, &sampler);
        }
        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &frame.rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(frame.width * 4),
                rows_per_image: Some(frame.height),
            },
            wgpu::Extent3d {
                width: frame.width,
                height: frame.height,
                depth_or_array_layers: 1,
            },
        );
        let output = match self.surface.get_current_texture() {
            Ok(t) => t,
            Err(wgpu::SurfaceError::Lost | wgpu::SurfaceError::Outdated) => {
                self.surface.configure(&self.device, &self.config);
                return Ok(());
            }
            Err(e) => anyhow::bail!("surface error: {e}"),
        };
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("present"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            let (x, y, w, h) = self.dest_rect();
            pass.set_viewport(x, y, w, h, 0.0, 1.0);
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        self.queue.submit(Some(encoder.finish()));
        self.window.pre_present_notify();
        output.present();
        Ok(())
    }
}

impl App {
    /// Window physical position -> logical viewport 24.8 coordinates.
    fn to_logical(&self, pos: (f64, f64)) -> (i32, i32) {
        let Some(gpu) = &self.gpu else {
            return (0, 0);
        };
        let (x, y, w, h) = gpu.dest_rect();
        let lx = (pos.0 as f32 - x) / w * self.viewport.0 as f32;
        let ly = (pos.1 as f32 - y) / h * self.viewport.1 as f32;
        ((lx * 256.0).round() as i32, (ly * 256.0).round() as i32)
    }

    fn drain_rpc(&mut self, event_loop: &ActiveEventLoop) {
        let Some(rx) = &self.rpc else { return };
        let lines: Vec<String> = rx.try_iter().collect();
        for line in lines {
            let handled = rpc::handle_line(&mut self.session, &line);
            if let Some(resp) = &handled.response
                && let Err(e) = rpc::write_response(resp)
            {
                eprintln!("opensherwood: rpc write failed: {e}");
            }
            if handled.shutdown {
                self.exit = true;
                event_loop.exit();
            }
        }
    }

    fn tick_if_due(&mut self) {
        // Controlled mode: with an RPC client attached the simulation advances only through `step`,
        // so identical scripts give identical hashes. Window input is queued for the next step.
        if self.rpc.is_some() {
            if !self.pending.is_empty() {
                self.session.queue_input(std::mem::take(&mut self.pending));
            }
            return;
        }
        let now = Instant::now();
        self.accumulator += now - self.last_tick;
        self.last_tick = now;
        let dt = Duration::from_secs(1) / TICK_RATE;
        // Cap catch-up so a stalled window does not spin through hundreds of ticks.
        if self.accumulator > dt * 10 {
            self.accumulator = dt * 10;
        }
        while self.accumulator >= dt {
            self.accumulator -= dt;
            let events = std::mem::take(&mut self.pending);
            self.session.tick(&events);
        }
    }

    fn redraw(&mut self) {
        let Some(gpu) = self.gpu.as_mut() else { return };
        if let Some(frame) = self.session.frame()
            && let Err(e) = gpu.present(frame)
        {
            eprintln!("opensherwood: present failed: {e}");
        }
    }
}

fn map_key(code: KeyCode) -> Option<Key> {
    Some(match code {
        KeyCode::Escape => Key::Escape,
        KeyCode::Space => Key::Space,
        KeyCode::ShiftLeft | KeyCode::ShiftRight => Key::Shift,
        KeyCode::ControlLeft | KeyCode::ControlRight => Key::Control,
        KeyCode::AltLeft | KeyCode::AltRight => Key::Alt,
        KeyCode::Tab => Key::Tab,
        KeyCode::Enter => Key::Enter,
        KeyCode::ArrowUp => Key::Up,
        KeyCode::ArrowDown => Key::Down,
        KeyCode::ArrowLeft => Key::Left,
        KeyCode::ArrowRight => Key::Right,
        KeyCode::KeyA => Key::Letter('a'),
        KeyCode::KeyB => Key::Letter('b'),
        KeyCode::KeyC => Key::Letter('c'),
        KeyCode::KeyD => Key::Letter('d'),
        KeyCode::KeyE => Key::Letter('e'),
        KeyCode::KeyF => Key::Letter('f'),
        KeyCode::KeyG => Key::Letter('g'),
        KeyCode::KeyH => Key::Letter('h'),
        KeyCode::KeyI => Key::Letter('i'),
        KeyCode::KeyJ => Key::Letter('j'),
        KeyCode::KeyK => Key::Letter('k'),
        KeyCode::KeyL => Key::Letter('l'),
        KeyCode::KeyM => Key::Letter('m'),
        KeyCode::KeyN => Key::Letter('n'),
        KeyCode::KeyO => Key::Letter('o'),
        KeyCode::KeyP => Key::Letter('p'),
        KeyCode::KeyQ => Key::Letter('q'),
        KeyCode::KeyR => Key::Letter('r'),
        KeyCode::KeyS => Key::Letter('s'),
        KeyCode::KeyT => Key::Letter('t'),
        KeyCode::KeyU => Key::Letter('u'),
        KeyCode::KeyV => Key::Letter('v'),
        KeyCode::KeyW => Key::Letter('w'),
        KeyCode::KeyX => Key::Letter('x'),
        KeyCode::KeyY => Key::Letter('y'),
        KeyCode::KeyZ => Key::Letter('z'),
        KeyCode::Digit0 => Key::Digit(0),
        KeyCode::Digit1 => Key::Digit(1),
        KeyCode::Digit2 => Key::Digit(2),
        KeyCode::Digit3 => Key::Digit(3),
        KeyCode::Digit4 => Key::Digit(4),
        KeyCode::Digit5 => Key::Digit(5),
        KeyCode::Digit6 => Key::Digit(6),
        KeyCode::Digit7 => Key::Digit(7),
        KeyCode::Digit8 => Key::Digit(8),
        KeyCode::Digit9 => Key::Digit(9),
        KeyCode::F1 => Key::Function(1),
        KeyCode::F2 => Key::Function(2),
        KeyCode::F3 => Key::Function(3),
        KeyCode::F4 => Key::Function(4),
        KeyCode::F5 => Key::Function(5),
        KeyCode::F6 => Key::Function(6),
        KeyCode::F7 => Key::Function(7),
        KeyCode::F8 => Key::Function(8),
        KeyCode::F9 => Key::Function(9),
        KeyCode::F10 => Key::Function(10),
        KeyCode::F11 => Key::Function(11),
        KeyCode::F12 => Key::Function(12),
        _ => return None,
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.gpu.is_some() {
            return;
        }
        let mut attrs = Window::default_attributes()
            .with_title("OpenSherwood")
            .with_inner_size(LogicalSize::new(
                self.viewport.0 * self.scale,
                self.viewport.1 * self.scale,
            ));
        if !self.windowed {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)));
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                eprintln!("opensherwood: cannot create window: {e}");
                event_loop.exit();
                return;
            }
        };
        match Gpu::new(window, self.viewport) {
            Ok(g) => self.gpu = Some(g),
            Err(e) => {
                eprintln!("opensherwood: cannot initialise graphics: {e:#}");
                event_loop.exit();
            }
        }
        self.last_tick = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if std::env::var_os("OPENSHERWOOD_TRACE_INPUT").is_some()
            && !matches!(event, WindowEvent::RedrawRequested)
        {
            eprintln!("opensherwood: event {event:?}");
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = self.gpu.as_mut() {
                    g.resize(size.width, size.height);
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                let PhysicalPosition { x, y } = position;
                self.pointer = (x, y);
                let (x256, y256) = self.to_logical(self.pointer);
                self.pending.push(InputEvent::PointerMove { x256, y256 });
            }
            WindowEvent::MouseInput { state, button, .. } => {
                if std::env::var_os("OPENSHERWOOD_TRACE_INPUT").is_some() {
                    eprintln!(
                        "opensherwood: mouse {button:?} {state:?} at {:?} -> logical {:?}",
                        self.pointer,
                        self.to_logical(self.pointer)
                    );
                }
                let button = match button {
                    MouseButton::Left => Button::Left,
                    MouseButton::Right => Button::Right,
                    MouseButton::Middle => Button::Middle,
                    _ => return,
                };
                self.pending.push(match state {
                    ElementState::Pressed => InputEvent::PointerDown { button },
                    ElementState::Released => InputEvent::PointerUp { button },
                });
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let steps = match delta {
                    MouseScrollDelta::LineDelta(_, y) => y.round() as i32,
                    MouseScrollDelta::PixelDelta(p) => (p.y / 40.0).round() as i32,
                };
                if steps != 0 {
                    self.pending.push(InputEvent::Wheel { delta: steps });
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if event.repeat {
                    return;
                }
                if event.state == ElementState::Pressed
                    && event.physical_key == PhysicalKey::Code(KeyCode::F11)
                    && let Some(g) = &self.gpu
                {
                    let fullscreen = g.window.fullscreen().is_none();
                    g.window.set_fullscreen(
                        fullscreen.then_some(winit::window::Fullscreen::Borderless(None)),
                    );
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key
                    && let Some(key) = map_key(code)
                {
                    self.pending.push(match event.state {
                        ElementState::Pressed => InputEvent::KeyDown { key },
                        ElementState::Released => InputEvent::KeyUp { key },
                    });
                }
            }
            WindowEvent::RedrawRequested => self.redraw(),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        if self.exit {
            return;
        }
        self.drain_rpc(event_loop);
        self.tick_if_due();
        if let Some(g) = &self.gpu {
            g.window.request_redraw();
        }
    }
}
