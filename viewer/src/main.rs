use std::borrow::Cow;

use g_code::emit::{Field, Token, Value};
use lyon_tessellation::{
    geom::{point, vector, Angle},
    path::{
        builder::WithSvg,
        traits::{Build, PathBuilder, SvgPathBuilder},
        ArcFlags, Path,
    },
};
use wasm_bindgen_futures::spawn_local;
use web_sys::HtmlCanvasElement;
use wgpu::{include_wgsl, util::DeviceExt, vertex_attr_array, SurfaceTarget};
use yew::{function_component, html, Html, MouseEvent, NodeRef, WheelEvent};

struct Viewer<'a> {
    surf: wgpu::Surface<'a>,
    dev: wgpu::Device,
    queue: wgpu::Queue,
    conf: wgpu::SurfaceConfiguration,
    render_pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
struct Vertex {
    pos: [f32; 3],
    color: [f32; 3],
}

impl Vertex {
    const DESC: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &vertex_attr_array![
            0 => Float32x3,
            1 => Float32x3,
        ],
    };
}

const VERTICES: &[Vertex] = &[
    Vertex {
        pos: [-0.0868241, 0.49240386, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // A
    Vertex {
        pos: [-0.49513406, 0.06958647, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // B
    Vertex {
        pos: [-0.21918549, -0.44939706, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // C
    Vertex {
        pos: [0.35966998, -0.3473291, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // D
    Vertex {
        pos: [0.44147372, 0.2347359, 0.0],
        color: [0.5, 0.0, 0.5],
    }, // E
];

const INDICES: &[u16] = &[0, 1, 4, 1, 2, 4, 2, 3, 4];

impl<'a> Viewer<'a> {
    async fn new(canvas: HtmlCanvasElement) -> Self {
        let width = canvas.width();
        let height = canvas.height();
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::default());
        let surf = instance
            .create_surface(SurfaceTarget::Canvas(canvas))
            .unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                compatible_surface: Some(&surf),
            })
            .await
            .unwrap();
        let (dev, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults(),
                },
                None,
            )
            .await
            .unwrap();
        let conf = surf.get_default_config(&adapter, width, height).unwrap();
        surf.configure(&dev, &conf);

        let shader = dev.create_shader_module(include_wgsl!("shader.wgsl"));
        let render_pipeline_layout = dev.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });
        let render_pipeline = dev.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None,
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::DESC],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: conf.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::all(),
                })],
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
        });

        let vertex_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: unsafe {
                std::slice::from_raw_parts(
                    VERTICES.as_ptr() as *const u8,
                    std::mem::size_of_val(VERTICES),
                )
            },
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = dev.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: None,
            contents: unsafe {
                std::slice::from_raw_parts(
                    INDICES.as_ptr() as *const u8,
                    std::mem::size_of_val(INDICES),
                )
            },
            usage: wgpu::BufferUsages::INDEX,
        });

        Self {
            surf,
            dev,
            queue,
            conf,
            render_pipeline,
            vertex_buffer,
            index_buffer,
        }
    }

    fn resize(&mut self, width: u32, height: u32) {
        self.conf.width = width;
        self.conf.height = height;
        self.surf.configure(&self.dev, &self.conf);
    }

    fn event(&mut self, event: Event) {}

    fn update(&mut self) {}

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surf.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .dev
            .create_command_encoder(&wgpu::CommandEncoderDescriptor::default());

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.1,
                            g: 0.2,
                            b: 0.3,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&self.render_pipeline);
            pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
        }

        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[derive(Debug, Clone)]
enum Event {
    MouseDown(MouseEvent),
    MouseMove(MouseEvent),
    MouseUp(MouseEvent),
    Wheel(WheelEvent),
}

#[function_component]
fn App() -> Html {
    let node_ref = NodeRef::default();

    let r = node_ref.clone();
    let viewer = async move {
        let mut viewer = Viewer::new(r.cast().unwrap()).await;
        viewer.render().unwrap();
    };
    spawn_local(viewer);
    html! {
        <canvas ref={node_ref.clone()} width="800" height="800"/>
    }
}

fn main() {
    yew::Renderer::<App>::new().render();
}

fn token_stream_to_path<'a>(tokens: impl IntoIterator<Item = Token<'a>>) -> Path {
    let mut movement_type = 0;
    let mut inches = false;
    let mut relative = false;
    let mut x = 0.;
    let mut y = 0.;
    let mut r = 0.;

    let mut builder = WithSvg::new(Path::builder());
    builder.move_to(point(0., 0.));
    for t in tokens {
        match t {
            Token::Field(Field {
                letters: Cow::Borrowed("G"),
                value: Value::Integer(i),
            }) => match i {
                0 | 1 | 2 | 3 => {
                    movement_type = i;
                }
                20 => {
                    inches = true;
                }
                21 => {
                    inches = false;
                }
                90 => {
                    relative = false;
                }
                91 => {
                    relative = true;
                }
                94 => {}
                _ => {}
            },
            Token::Field(Field {
                letters: Cow::Borrowed("M"),
                value: Value::Integer(i),
            }) => match i {
                2 => {
                    break;
                }
                _ => {}
            },
            Token::Field(Field {
                letters: Cow::Borrowed(x_or_y @ ("X" | "Y")),
                value,
            }) => {
                if let Some(mut v) = value.as_f64() {
                    let dest = if x_or_y == "X" { &mut x } else { &mut y };

                    if inches {
                        v = uom::si::f64::Length::new::<uom::si::length::inch>(v)
                            .get::<uom::si::length::millimeter>();
                    }

                    *dest = if relative { *dest + v } else { *dest };
                    match movement_type {
                        0 | 1 => {
                            builder.line_to(point(x as f32, y as f32));
                        }
                        2 | 3 => {
                            builder.arc_to(
                                vector(r as f32, r as f32),
                                Angle::zero(),
                                ArcFlags {
                                    large_arc: false,
                                    // 3 = counterclockwise, sweep
                                    sweep: movement_type == 3,
                                },
                                point(x as f32, y as f32),
                            );
                        }
                        _ => unreachable!(),
                    }
                }
            }
            Token::Field(Field { letters, value }) => {}
            Token::Comment { is_inline, inner } => {}
        }
    }
    builder.build()
}
