mod convert_path;
mod fill;
mod stroke;
use fill::iterate_fill;
pub use glam;
use glam::{DMat4, DVec2, Vec2, Vec4};
use lyon::lyon_tessellation::{FillVertex, StrokeVertex, VertexBuffers};
pub use roxmltree;
use roxmltree::Node;
use std::sync::Arc;
use stroke::iterate_stroke;
pub use usvg;
use usvg::{fontdb::Source, NodeKind, Path, Tree};
use xmlwriter::XmlWriter;

#[derive(PartialEq, PartialOrd, Eq, Ord, Clone, Copy, Debug)]
pub enum IndicesPriority {
    Fixed,
    Variable,
}

pub struct Callback<'a> {
    func: Box<dyn FnMut(&Node) -> IndicesPriority + 'a>,
}

impl<'a> Callback<'a> {
    pub fn new(c: impl FnMut(&Node) -> IndicesPriority + 'a) -> Self {
        Self { func: Box::new(c) }
    }
    fn process_events(&mut self, node: &Node) -> IndicesPriority {
        (self.func)(node)
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub transform_matrix_index: u32,
    pub color: [f32; 4],
}
impl From<&DVec2> for Vertex {
    fn from(v: &DVec2) -> Self {
        Self {
            position: [(v.x) as f32, (v.y) as f32, 0.0],
            ..Default::default()
        }
    }
}
impl From<(&FillVertex<'_>, &Vec4)> for Vertex {
    fn from((v, c): (&FillVertex, &Vec4)) -> Self {
        Self {
            position: [v.position().x, v.position().y, 0.],
            color: c.to_array(),
            ..Default::default()
        }
    }
}
impl From<(&StrokeVertex<'_, '_>, &Vec4)> for Vertex {
    fn from((v, c): (&StrokeVertex, &Vec4)) -> Self {
        Self {
            position: [v.position().x, v.position().y, 0.],
            color: c.to_array(),
            ..Default::default()
        }
    }
}

impl From<(&DVec2, &Vec4)> for Vertex {
    fn from((v, c): (&DVec2, &Vec4)) -> Self {
        Self {
            position: [(v.x) as f32, (v.y) as f32, 0.0],
            color: [c.x, c.y, c.z, c.w],
            ..Default::default()
        }
    }
}

pub type Index = u32;
pub type Vertices = Vec<Vertex>;
pub type Indices = Vec<Index>;
pub type DrawPrimitives = (Vertices, Indices);
pub type Size = Vec2;
pub type Position = Vec2;
pub type Rect = (Position, Size);

pub const FALLBACK_COLOR: Vec4 = Vec4::ONE;

struct TransformVariable {
    transform: DMat4,
    transform_index: u16,
}

#[derive(Clone, Default, Debug)]
struct Geometry {
    id: Option<String>,
    vertices: Vertices,
    indices: Indices,
    index_base: usize,
    transform_index: usize,
}

#[derive(Clone, Default, Debug)]
struct GeometryCollection {
    fixed_geometries: Geometries,
    variable_geometries: Geometries,
    fixed_geometries_vertices_len: usize,
    variable_geometries_vertices_len: usize,
}

impl GeometryCollection {
    fn get_indices(&self) -> Indices {
        [
            self.fixed_geometries.get_indices_with_offset(0),
            self.variable_geometries
                .get_indices_with_offset(self.fixed_geometries_vertices_len as u32),
        ]
        .concat()
    }
    fn get_vertices(&self) -> Vertices {
        [
            self.fixed_geometries.get_vertices(),
            self.variable_geometries.get_vertices(),
        ]
        .concat()
    }
    fn get_vertices_len(&self, priority: IndicesPriority) -> usize {
        match priority {
            IndicesPriority::Fixed => self.fixed_geometries_vertices_len,
            IndicesPriority::Variable => self.variable_geometries_vertices_len,
        }
    }
    fn push_with_priority(&mut self, geometry: Geometry, priority: IndicesPriority) {
        let (geometries, vertices_len) = match priority {
            IndicesPriority::Fixed => (
                &mut self.fixed_geometries,
                &mut self.fixed_geometries_vertices_len,
            ),
            IndicesPriority::Variable => (
                &mut self.variable_geometries,
                &mut self.variable_geometries_vertices_len,
            ),
        };
        *vertices_len += geometry.get_vertices_len();
        geometries.0.push(geometry);
    }
}

#[derive(Clone, Default, Debug)]
struct Geometries(Vec<Geometry>);
impl Geometries {
    fn get_vertices(&self) -> Vertices {
        self.0.iter().flat_map(|v| v.get_v()).collect()
    }
    fn get_indices_with_offset(&self, offset: u32) -> Indices {
        self.0
            .iter()
            .flat_map(|v| v.get_i().iter().map(|i| i + offset).collect::<Indices>())
            .collect()
    }
}

impl Geometry {
    fn get_vertices_len(&self) -> usize {
        self.vertices.len()
    }
    fn get_v(&self) -> Vertices {
        self.vertices.clone()
    }
    fn get_i(&self) -> Indices {
        self.indices
            .iter()
            .map(|index| index + self.index_base as u32)
            .collect()
    }
}

impl Geometry {
    fn prepare_vertex_buffer(p: &Path) -> VertexBuffers<Vertex, Index> {
        let mut vertex_buffer = VertexBuffers::<Vertex, Index>::new();
        if let Some(ref stroke) = p.stroke {
            let color = match stroke.paint {
                usvg::Paint::Color(c) => Vec4::new(
                    c.red as f32 / u8::MAX as f32,
                    c.green as f32 / u8::MAX as f32,
                    c.blue as f32 / u8::MAX as f32,
                    stroke.opacity.value() as f32,
                ),
                _ => FALLBACK_COLOR,
            };
            iterate_stroke(stroke, p, &mut vertex_buffer, color);
        }
        if let Some(ref fill) = p.fill {
            let color = match fill.paint {
                usvg::Paint::Color(c) => Vec4::new(
                    c.red as f32 / u8::MAX as f32,
                    c.green as f32 / u8::MAX as f32,
                    c.blue as f32 / u8::MAX as f32,
                    fill.opacity.value() as f32,
                ),
                _ => FALLBACK_COLOR,
            };

            iterate_fill(p, &color, &mut vertex_buffer);
        };
        return vertex_buffer;
    }
    fn new(p: &Path, index_base: usize, id: Option<String>) -> Self {
        let v = Self::prepare_vertex_buffer(p);
        Self {
            id,
            vertices: v.vertices,
            indices: v.indices,
            index_base,
            ..Default::default()
        }
    }
}

fn recursive_svg(
    node: usvg::Node,
    priority: IndicesPriority,
    geometry_set: &mut GeometryCollection,
) {
    if let usvg::NodeKind::Path(ref p) = *node.borrow() {
        let geometry = Geometry::new(
            &p,
            geometry_set.get_vertices_len(priority),
            if &p.id == "" {
                None
            } else {
                Some((*p.id).to_string())
            },
        );
        geometry_set.push_with_priority(geometry, priority)
    }
    for child in node.children() {
        recursive_svg(child, priority, geometry_set);
    }
}

fn recursive_xml(
    writer_without_siblings: &mut XmlWriter,
    writer_with_siblings: &mut XmlWriter,
    node: roxmltree::Node,
    parent_priority: IndicesPriority,
    callback: &mut Callback,
    geometry_set: &mut GeometryCollection,
    usvg_options: &usvg::Options,
) {
    let priority = parent_priority.max(callback.process_events(&node));
    let writer = if priority > parent_priority {
        writer_without_siblings
    } else {
        writer_with_siblings
    };
    if node.is_element() {
        writer.start_element(node.tag_name().name());
        for a in node.attributes() {
            let name = if let Some(namespace) = a.namespace() {
                format!("xml:{}", a.name())
            } else {
                a.name().to_string()
            };
            writer.write_attribute(&name, a.value());
        }
        if node.tag_name().name() == "svg" {
            writer.write_attribute("xmlns", "http://www.w3.org/2000/svg");
        }
    }
    if node.is_text() {
        writer.write_text(node.text().unwrap());
    }

    for child in node.children() {
        recursive_xml(
            &mut writer.clone(),
            writer,
            child,
            priority,
            callback,
            geometry_set,
            &usvg_options,
        );
    }
    if node.is_element() {
        writer.end_element();
    }
}

pub fn init(mut callback: Callback) -> (DrawPrimitives, Rect) {
    // Parse and tessellate the geometry

    let contents = include_bytes!("../fallback_font/Roboto-Medium.ttf");
    let mut opt = usvg::Options::default();
    opt.fontdb
        .load_font_source(Source::Binary(Arc::new(contents.as_ref())));
    opt.font_family = "Roboto Medium".to_string();
    let tree = roxmltree::Document::parse(include_str!("../../svg/life_text.svg")).unwrap();

    let mut geometry_set = GeometryCollection::default();

    let mut writer = xmlwriter::XmlWriter::new(xmlwriter::Options {
        indent: xmlwriter::Indent::Spaces(0),
        ..Default::default()
    });
    writer.write_declaration();
    for node in tree.root().children() {
        recursive_xml(
            &mut writer.clone(),
            &mut writer,
            node,
            IndicesPriority::Fixed,
            &mut callback,
            &mut geometry_set,
            &opt,
        )
    }
    let xml = writer.end_document();
    println!("{}", xml);
    let rtree = Tree::from_str(&xml, &opt.to_ref()).unwrap();
    recursive_svg(rtree.root(), IndicesPriority::Fixed, &mut geometry_set);
    let view_box = rtree.svg_node().view_box;
    let rect: Rect = (
        Vec2::new(view_box.rect.x() as f32, view_box.rect.y() as f32),
        Vec2::new(view_box.rect.width() as f32, view_box.rect.height() as f32),
    );

    (
        (geometry_set.get_vertices(), geometry_set.get_indices()),
        rect,
    )
}
