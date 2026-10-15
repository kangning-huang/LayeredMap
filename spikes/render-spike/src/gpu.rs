//! Strategy C: the data layers drawn on the GPU every frame, through an egui paint callback.
//!
//! Geometry is uploaded once, in Web Mercator metres relative to an origin near the data (so
//! `f32` keeps millimetre precision). Each frame only a small uniform changes: the camera's
//! centre and the pixels per metre. Points are instanced quads shaded as discs; polygons are
//! triangulated once (fill) and drawn again as a line list (outline).

use eframe::egui_wgpu::{self, wgpu};
use rustgis_data::geometry::Geometry;
use rustgis_geom::{Coord, lonlat_to_merc};

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    /// Camera centre minus the origin, metres.
    center: [f32; 2],
    /// Viewport size in physical pixels.
    screen: [f32; 2],
    /// Physical pixels per metre.
    k: f32,
    /// Point diameter in physical pixels.
    point_px: f32,
    _pad: [f32; 2],
    point_color: [f32; 4],
    fill_color: [f32; 4],
    line_color: [f32; 4],
}

const SHADER: &str = r#"
struct U {
    center: vec2<f32>,
    screen: vec2<f32>,
    k: f32,
    point_px: f32,
    pad: vec2<f32>,
    point_color: vec4<f32>,
    fill_color: vec4<f32>,
    line_color: vec4<f32>,
};
@group(0) @binding(0) var<uniform> u: U;

fn to_clip(p: vec2<f32>) -> vec4<f32> {
    let px = (p - u.center) * u.k;
    return vec4<f32>(px.x / (u.screen.x * 0.5), px.y / (u.screen.y * 0.5), 0.0, 1.0);
}

struct PointOut { @builtin(position) pos: vec4<f32>, @location(0) local: vec2<f32> };

@vertex fn vs_point(@builtin(vertex_index) vi: u32, @location(0) p: vec2<f32>) -> PointOut {
    var corners = array<vec2<f32>, 6>(
        vec2(-1.0, -1.0), vec2(1.0, -1.0), vec2(1.0, 1.0),
        vec2(-1.0, -1.0), vec2(1.0, 1.0), vec2(-1.0, 1.0));
    let c = corners[vi];
    var o: PointOut;
    let base = to_clip(p);
    let half = (u.point_px * 0.5 + 0.5) / (u.screen * 0.5);
    o.pos = vec4<f32>(base.xy + c * half, 0.0, 1.0);
    o.local = c * (u.point_px * 0.5 + 0.5);
    return o;
}
@fragment fn fs_point(i: PointOut) -> @location(0) vec4<f32> {
    let r = u.point_px * 0.5;
    let a = clamp(r - length(i.local) + 0.5, 0.0, 1.0);
    return u.point_color * a;
}

@vertex fn vs_flat(@location(0) p: vec2<f32>) -> @builtin(position) vec4<f32> { return to_clip(p); }
@fragment fn fs_fill() -> @location(0) vec4<f32> { return u.fill_color; }
@fragment fn fs_line() -> @location(0) vec4<f32> { return u.line_color; }
"#;

pub struct GpuData {
    pipe_points: wgpu::RenderPipeline,
    pipe_fill: wgpu::RenderPipeline,
    pipe_line: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind: wgpu::BindGroup,
    points: wgpu::Buffer,
    n_points: u32,
    fill_vertices: wgpu::Buffer,
    fill_indices: wgpu::Buffer,
    n_fill_indices: u32,
    lines: wgpu::Buffer,
    n_line_vertices: u32,
}

/// What the CPU side hands over: geometry in metres relative to `origin`.
pub struct Mesh {
    pub points: Vec<[f32; 2]>,
    pub fill_vertices: Vec<[f32; 2]>,
    pub fill_indices: Vec<u32>,
    pub lines: Vec<[f32; 2]>,
}

impl Mesh {
    pub fn build(origin: Coord, layers: &[&rustgis_data::Dataset]) -> Mesh {
        let rel = |c: Coord| {
            let m = lonlat_to_merc(c);
            [(m.x - origin.x) as f32, (m.y - origin.y) as f32]
        };
        let mut mesh = Mesh { points: Vec::new(), fill_vertices: Vec::new(), fill_indices: Vec::new(), lines: Vec::new() };
        for ds in layers {
            for f in &ds.features {
                match &f.geometry {
                    Some(Geometry::Point(c)) => mesh.points.push(rel(*c)),
                    Some(Geometry::MultiPoint(cs)) => mesh.points.extend(cs.iter().map(|c| rel(*c))),
                    Some(Geometry::Polygon(parts)) => {
                        for rings in parts {
                            let open = |r: &Vec<Coord>| -> Vec<(f64, f64)> {
                                let mut v: Vec<(f64, f64)> = r
                                    .iter()
                                    .map(|c| {
                                        let p = rel(*c);
                                        (f64::from(p[0]), f64::from(p[1]))
                                    })
                                    .collect();
                                if v.len() > 1 && v.first() == v.last() {
                                    v.pop();
                                }
                                v
                            };
                            let Some(outer) = rings.first().map(open) else { continue };
                            let holes: Vec<Vec<(f64, f64)>> = rings.iter().skip(1).map(open).collect();
                            let base = mesh.fill_vertices.len() as u32;
                            if let Some(tris) = rustgis_geom::triangulate::triangulate(&outer, &holes) {
                                mesh.fill_vertices.extend(outer.iter().chain(holes.iter().flatten()).map(|&(x, y)| [x as f32, y as f32]));
                                mesh.fill_indices.extend(tris.iter().flat_map(|t| t.iter().map(|&i| base + i as u32)));
                            }
                            for ring in std::iter::once(&outer).chain(holes.iter()) {
                                for (i, a) in ring.iter().enumerate() {
                                    let b = ring[(i + 1) % ring.len()];
                                    mesh.lines.push([a.0 as f32, a.1 as f32]);
                                    mesh.lines.push([b.0 as f32, b.1 as f32]);
                                }
                            }
                        }
                    }
                    Some(Geometry::Line(parts)) => {
                        for part in parts {
                            for w in part.windows(2) {
                                mesh.lines.push(rel(w[0]));
                                mesh.lines.push(rel(w[1]));
                            }
                        }
                    }
                    None => {}
                }
            }
        }
        mesh
    }
}

fn buffer(device: &wgpu::Device, label: &str, data: &[u8], usage: wgpu::BufferUsages) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;
    // An empty buffer is invalid: keep at least 16 bytes.
    let pad = [0u8; 16];
    let contents = if data.is_empty() { &pad[..] } else { data };
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some(label), contents, usage })
}

impl GpuData {
    pub fn new(rs: &egui_wgpu::RenderState, mesh: &Mesh) -> GpuData {
        let device = &rs.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("lm-spike"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("lm-spike uniforms"),
            size: std::mem::size_of::<Uniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: false, min_binding_size: None },
                count: None,
            }],
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: uniforms.as_entire_binding() }],
        });
        let pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let target = [Some(wgpu::ColorTargetState {
            format: rs.target_format,
            blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
            write_mask: wgpu::ColorWrites::ALL,
        })];
        let vb = |step| wgpu::VertexBufferLayout {
            array_stride: 8,
            step_mode: step,
            attributes: &[wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x2, offset: 0, shader_location: 0 }],
        };
        let pipeline = |vs: &str, fs: &str, step, topology| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(fs),
                layout: Some(&pl),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some(vs),
                    buffers: &[Some(vb(step))],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some(fs),
                    targets: &target,
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState { topology, ..Default::default() },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let pipe_points = pipeline("vs_point", "fs_point", wgpu::VertexStepMode::Instance, wgpu::PrimitiveTopology::TriangleList);
        let pipe_fill = pipeline("vs_flat", "fs_fill", wgpu::VertexStepMode::Vertex, wgpu::PrimitiveTopology::TriangleList);
        let pipe_line = pipeline("vs_flat", "fs_line", wgpu::VertexStepMode::Vertex, wgpu::PrimitiveTopology::LineList);
        let v = wgpu::BufferUsages::VERTEX;
        GpuData {
            pipe_points,
            pipe_fill,
            pipe_line,
            uniforms,
            bind,
            points: buffer(device, "points", bytemuck::cast_slice(&mesh.points), v),
            n_points: mesh.points.len() as u32,
            fill_vertices: buffer(device, "fill v", bytemuck::cast_slice(&mesh.fill_vertices), v),
            fill_indices: buffer(device, "fill i", bytemuck::cast_slice(&mesh.fill_indices), wgpu::BufferUsages::INDEX),
            n_fill_indices: mesh.fill_indices.len() as u32,
            lines: buffer(device, "lines", bytemuck::cast_slice(&mesh.lines), v),
            n_line_vertices: mesh.lines.len() as u32,
        }
    }
}

/// One frame's draw of the uploaded data at a camera.
pub struct DrawData {
    pub center_rel: [f32; 2],
    /// The map rect in physical pixels, and the pixels per point.
    pub viewport_px: [f32; 2],
    pub ppp: f32,
    /// Logical points per metre (1 / camera resolution).
    pub points_per_m: f32,
    pub point_pt: f32,
    pub point_color: [f32; 4],
    pub fill_color: [f32; 4],
    pub line_color: [f32; 4],
}

/// Premultiplies a straight-alpha sRGB colour for the premultiplied blend.
pub fn premul(rgb: [u8; 3], a: f32) -> [f32; 4] {
    [f32::from(rgb[0]) / 255.0 * a, f32::from(rgb[1]) / 255.0 * a, f32::from(rgb[2]) / 255.0 * a, a]
}

impl egui_wgpu::CallbackTrait for DrawData {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        res: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if let Some(g) = res.get::<GpuData>() {
            let u = Uniforms {
                center: self.center_rel,
                screen: self.viewport_px,
                k: self.points_per_m * self.ppp,
                point_px: self.point_pt * self.ppp,
                _pad: [0.0; 2],
                point_color: self.point_color,
                fill_color: self.fill_color,
                line_color: self.line_color,
            };
            queue.write_buffer(&g.uniforms, 0, bytemuck::bytes_of(&u));
        }
        Vec::new()
    }

    fn paint(&self, _info: egui::PaintCallbackInfo, pass: &mut wgpu::RenderPass<'static>, res: &egui_wgpu::CallbackResources) {
        let Some(g) = res.get::<GpuData>() else { return };
        pass.set_bind_group(0, &g.bind, &[]);
        if g.n_fill_indices > 0 {
            pass.set_pipeline(&g.pipe_fill);
            pass.set_vertex_buffer(0, g.fill_vertices.slice(..));
            pass.set_index_buffer(g.fill_indices.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..g.n_fill_indices, 0, 0..1);
        }
        if g.n_line_vertices > 0 {
            pass.set_pipeline(&g.pipe_line);
            pass.set_vertex_buffer(0, g.lines.slice(..));
            pass.draw(0..g.n_line_vertices, 0..1);
        }
        if g.n_points > 0 {
            pass.set_pipeline(&g.pipe_points);
            pass.set_vertex_buffer(0, g.points.slice(..));
            pass.draw(0..6, 0..g.n_points);
        }
    }
}
