use crate::{
    callback::{IndicesPriority, InitCallback, PassDown},
    prepare_vertex_buffer::prepare_vertex_buffer,
};
use guppies::{
    glam::Vec2,
    primitives::{Indices, Rect, Vertices},
};
use roxmltree::{Document, NodeId};
use std::{collections::HashMap, ops::Range, sync::Arc};
use usvg::{fontdb::Source, NodeKind, Options, Path, PathBbox, Tree};
use xmlwriter::XmlWriter;

fn rect_from_bbox(bbox: &PathBbox) -> Rect {
    Rect {
        position: Vec2::new(bbox.x() as f32, bbox.y() as f32),
        size: Vec2::new(bbox.width() as f32, bbox.height() as f32),
    }
}

#[derive(Clone, Debug, Default)]
pub struct Geometry {
    vertices: Vertices,
    indices: Indices,
    priority: IndicesPriority,
}
impl Geometry {
    pub fn get_vertices_len(&self) -> usize {
        self.vertices.len()
    }
    pub fn get_v(&self) -> Vertices {
        self.vertices.clone()
    }
    pub fn get_i_with_offset(&self, offset: u32) -> Indices {
        self.indices.iter().map(|index| index + offset).collect()
    }
    pub fn new(p: &Path, transform_id: u32, priority: IndicesPriority) -> Self {
        let v = prepare_vertex_buffer(p, transform_id);
        Self {
            vertices: v.vertices,
            indices: v.indices,
            priority,
        }
    }
}

fn recursive_svg(
    node: usvg::Node,
    pass_down: PassDown,
    geometries: &mut Vec<Geometry>,
    callback: &mut InitCallback,
) {
    let (geometry, pass_down) = callback.process_events(&(node.clone(), pass_down));
    if let Some(geometry) = geometry {
        geometries.push(geometry);
    }

    for child in node.children() {
        recursive_svg(child, pass_down.clone(), geometries, callback);
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
    pub geometries: Vec<Geometry>,
    pub document: roxmltree::Document<'a>,
    pub id_map: HashMap<String, NodeId>,
    pub bbox: Rect,
    usvg_options: Options,
}
impl<'a> Default for SvgSet<'a> {
    fn default() -> Self {
        Self {
            geometries: vec![],
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
    pub fn get_vertices(&self) -> Vertices {
        todo!()
    }
    pub fn get_indices(&self) -> Indices {
        todo!()
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
        let mut geometries: Vec<Geometry> = vec![];
        recursive_svg(
            tree.root(),
            PassDown::default(),
            &mut geometries,
            &mut callback,
        );
        let view_box = tree.svg_node().view_box;
        let bbox: Rect = Rect::new(
            Vec2::new(view_box.rect.x() as f32, view_box.rect.y() as f32),
            Vec2::new(view_box.rect.width() as f32, view_box.rect.height() as f32),
        );
        Self {
            geometries,
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
    }
}
