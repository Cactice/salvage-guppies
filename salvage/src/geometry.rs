use crate::{
    callback::{IndicesPriority, InitCallback, Initialization},
    fill::iterate_fill,
    stroke::iterate_stroke,
};
use guppies::{
    glam::{Vec2, Vec4},
    primitives::{Index, Indices, Rect, Vertex, Vertices},
};
use lyon::lyon_tessellation::VertexBuffers;
use roxmltree::{Document, NodeId};
use std::{collections::HashMap, iter, ops::Range, sync::Arc};
use usvg::{fontdb::Source, NodeKind, Options, Path, PathBbox, Tree};
use xmlwriter::XmlWriter;
pub const FALLBACK_COLOR: Vec4 = Vec4::ONE;

fn rect_from_bbox(bbox: &PathBbox) -> Rect {
    Rect {
        position: Vec2::new(bbox.x() as f32, bbox.y() as f32),
        size: Vec2::new(bbox.width() as f32, bbox.height() as f32),
    }
}

#[derive(Clone, Default, Debug)]
pub struct GeometrySet {
    fixed_geometries: Geometries,
    variable_geometries: Geometries,
    fixed_geometries_vertices_len: usize,
    variable_geometries_vertices_len: usize,
    variable_geometries_id_range: HashMap<String, Range<usize>>,
    transform_count: u32,
}

impl GeometrySet {
    pub fn get_geometries_at_position(&self, position: &Vec2) -> Geometries {
        // TODO: The performance can be improved so much by only checking clickable
        // but IDK how to keep a reference of such Geometries and I imagine this is fast enough
        Geometries(
            iter::empty::<&Geometry>()
                .chain(self.fixed_geometries.0.iter())
                .chain(self.variable_geometries.0.iter())
                .filter(|g| g.bbox.contains_point(position))
                .cloned()
                .collect(),
        )
    }
    fn update_geometry(&mut self, id: &String, vertices: Vertices, indices: Indices) {
        let variable_geometry_index = self
            .variable_geometries_id_range
            .get(id)
            .expect("Invalid id for variable_geometries_id_range")
            .start;
        let geometry = self
            .variable_geometries
            .0
            .get_mut(variable_geometry_index)
            .expect("variable_geometry_index is out of bounds");

        let index_base_offset = vertices.len() as i32 - geometry.vertices.len() as i32;
        geometry.vertices = vertices;
        geometry.indices = indices;
        self.variable_geometries.0[variable_geometry_index + 1..]
            .iter_mut()
            .for_each(|geometry: &mut Geometry| {
                geometry.index_base =
                    (geometry.index_base as i32 + index_base_offset as i32) as usize;
            });
    }
    pub fn get_indices(&self) -> Indices {
        [
            self.fixed_geometries.get_indices_with_offset(0),
            self.variable_geometries
                .get_indices_with_offset(self.fixed_geometries_vertices_len as u32),
        ]
        .concat()
    }
    pub fn get_vertices(&self) -> Vertices {
        [
            self.fixed_geometries.get_vertices(),
            self.variable_geometries.get_vertices(),
        ]
        .concat()
    }
    pub fn get_vertices_len(&self, priority: IndicesPriority) -> usize {
        match priority {
            IndicesPriority::Fixed => self.fixed_geometries_vertices_len,
            IndicesPriority::Variable => self.variable_geometries_vertices_len,
        }
    }
    pub fn push_with_priority(&mut self, geometry: Geometry, priority: IndicesPriority) {
        if priority == IndicesPriority::Variable {
            geometry.ids.iter().for_each(|id| {
                let start = if let Some(range) = self.variable_geometries_id_range.get(id) {
                    range.start
                } else {
                    self.variable_geometries.0.len()
                };
                let end = self.variable_geometries.0.len() + 1;
                let new_range = start..end;
                self.variable_geometries_id_range
                    .insert(id.to_string(), new_range);
            });
        }
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
pub struct Geometries(pub Vec<Geometry>);
impl Geometries {
    pub fn get_tag_names(&self) -> Vec<Vec<String>> {
        self.0.iter().map(|a| a.ids.clone()).collect()
    }
    pub fn get_vertices(&self) -> Vertices {
        self.0.iter().flat_map(|v| v.get_v()).collect()
    }
    pub fn get_indices_with_offset(&self, offset: u32) -> Indices {
        self.0
            .iter()
            .flat_map(|v| v.get_i().iter().map(|i| i + offset).collect::<Indices>())
            .collect()
    }
}

#[derive(Clone, Debug, Default)]
pub struct Geometry {
    ids: Vec<String>,
    vertices: Vertices,
    indices: Indices,
    index_base: usize,
    transform_id: u32,
    bbox: Rect,
}
impl Geometry {
    pub fn get_vertices_len(&self) -> usize {
        self.vertices.len()
    }
    pub fn get_v(&self) -> Vertices {
        self.vertices.clone()
    }
    pub fn get_i(&self) -> Indices {
        self.indices
            .iter()
            .map(|index| index + self.index_base as u32)
            .collect()
    }
    pub fn prepare_vertex_buffer(p: &Path, transform_id: u32) -> VertexBuffers<Vertex, Index> {
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
            iterate_stroke(stroke, p, &mut vertex_buffer, color, transform_id);
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

            iterate_fill(p, &color, &mut vertex_buffer, transform_id);
        };
        vertex_buffer
    }
    pub fn new(p: &Path, index_base: usize, ids: Vec<String>, transform_id: u32) -> Self {
        let v = Self::prepare_vertex_buffer(p, transform_id);
        Self {
            ids,
            vertices: v.vertices,
            indices: v.indices,
            index_base,
            transform_id,
            bbox: rect_from_bbox(&p.data.bbox().unwrap()),
        }
    }
}

fn recursive_svg(
    node: usvg::Node,
    parent_priority: IndicesPriority,
    callback: &mut InitCallback,
    geometry_set: &mut GeometrySet,
    mut ids: Vec<String>,
    parent_transform_id: u32,
) {
    let priority = parent_priority.max(callback.process_events(&node).indices_priority);
    let node_ref = &node.borrow();
    let id = NodeKind::id(node_ref);
    if !id.is_empty() {
        ids.push(id.to_string());
    }

    // TODO: DI
    let transform_id = if id.ends_with("#dynamic") {
        geometry_set.transform_count += 1;
        geometry_set.transform_count
    } else {
        parent_transform_id
    };
    if let usvg::NodeKind::Path(ref p) = *node.borrow() {
        let geometry = Geometry::new(
            p,
            geometry_set.get_vertices_len(priority),
            ids.to_vec(),
            transform_id,
        );
        geometry_set.push_with_priority(geometry, priority);
    }
    for child in node.children() {
        recursive_svg(
            child,
            priority,
            callback,
            geometry_set,
            ids.clone(),
            transform_id,
        );
    }
}

fn find_text_node_path(node: roxmltree::Node, path: &mut Vec<roxmltree::NodeId>) -> bool {
    if node.is_text() {
        return true;
    }
    for child in node.children() {
        if find_text_node_path(child, path) {
            if child.is_element() {
                path.push(child.id());
            }
            return true;
        }
    }
    false
}

#[derive(Debug)]
pub struct SvgSet<'a> {
    pub geometry_set: GeometrySet,
    pub document: roxmltree::Document<'a>,
    pub id_map: HashMap<String, NodeId>,
    pub bbox: Rect,
    usvg_options: Options,
}
impl<'a> Default for SvgSet<'a> {
    fn default() -> Self {
        Self {
            geometry_set: Default::default(),
            document: Document::parse("<e/>").unwrap(),
            id_map: Default::default(),
            bbox: Default::default(),
            usvg_options: Default::default(),
        }
    }
}
impl<'a> SvgSet<'a> {
    fn copy_element(&self, node: &roxmltree::Node, writer: &mut XmlWriter) {
        writer.start_element(node.tag_name().name());
        for a in node.attributes() {
            let name = if a.namespace().is_some() {
                format!("xml:{}", a.name())
            } else {
                a.name().to_string()
            };
            if a.name() != "filter" {
                writer.write_attribute(&name, a.value());
            }
        }
    }
    pub fn get_node_with_id(&self, id: &String) -> Result<roxmltree::Node, &str> {
        let node_id = self.id_map.get(id).ok_or("Not in node_id")?;
        let node = self.document.get_node(*node_id).ok_or("Not in document")?;
        Ok(node)
    }
    pub fn new(xml: &'a str, mut callback: InitCallback) -> Self {
        let font = include_bytes!("../fallback_font/Roboto-Medium.ttf");
        let mut opt = Options::default();
        opt.fontdb
            .load_font_source(Source::Binary(Arc::new(font.as_ref())));
        opt.font_family = "Roboto Medium".to_string();
        opt.keep_named_groups = true;
        let mut geometry_set = GeometrySet {
            transform_count: 1,
            ..Default::default()
        };
        let document = Document::parse(xml).unwrap();
        let tree = Tree::from_xmltree(&document, &opt.to_ref()).unwrap();
        let id_map =
            document
                .descendants()
                .fold(HashMap::<String, NodeId>::new(), |mut acc, curr| {
                    if let Some(attribute_id) = curr.attribute("id") {
                        acc.insert(attribute_id.to_string(), curr.id());
                    }
                    acc
                });
        recursive_svg(
            tree.root(),
            IndicesPriority::Fixed,
            &mut callback,
            &mut geometry_set,
            vec![],
            1,
        );
        let view_box = tree.svg_node().view_box;
        let bbox: Rect = Rect::new(
            Vec2::new(view_box.rect.x() as f32, view_box.rect.y() as f32),
            Vec2::new(view_box.rect.width() as f32, view_box.rect.height() as f32),
        );
        Self {
            geometry_set,
            document,
            id_map,
            bbox,
            usvg_options: opt,
        }
    }
    fn get_base_writer(&self) -> XmlWriter {
        let mut writer = XmlWriter::new(xmlwriter::Options {
            use_single_quote: true,
            ..Default::default()
        });
        writer.write_declaration();
        writer.set_preserve_whitespaces(true);
        writer
    }
    pub fn update_text(&mut self, id: &String, new_text: &String) {
        let node = self.get_node_with_id(id).unwrap();
        let mut writer = self.get_base_writer();
        let mut parent_ids: Vec<roxmltree::NodeId> = vec![];
        find_text_node_path(node, &mut parent_ids);

        parent_ids.push(node.id());
        let mut current_node = node;
        while let Some(parent) = current_node.parent() {
            if !parent.is_element() {
                if parent.parent().is_none() {
                    break;
                }
                continue;
            }
            parent_ids.push(parent.id());
            current_node = parent;
        }
        while let Some(parent_id) = parent_ids.pop() {
            let parent = self.document.get_node(parent_id).unwrap();
            self.copy_element(&parent, &mut writer);
            if parent.has_tag_name("svg") {
                writer.write_attribute("xmlns", "http://www.w3.org/2000/svg");
            }
        }
        writer.write_text(new_text);

        let xml = writer.end_document();
        let tree = Tree::from_str(&xml, &self.usvg_options.to_ref()).unwrap();
        let mut geometry_set = GeometrySet::default();
        recursive_svg(
            tree.root(),
            IndicesPriority::Variable,
            &mut InitCallback::new(|_| Initialization::default()),
            &mut geometry_set,
            vec![],
            1,
        );
        if let Some(Geometry {
            indices, vertices, ..
        }) = geometry_set.variable_geometries.0.first()
        {
            self.geometry_set
                .update_geometry(id, vertices.to_vec(), indices.to_vec());
        }
    }
}