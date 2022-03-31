mod line_to_parallel_lines;
use glam::DVec2;
use line_to_parallel_lines::line_to_parallel_lines;
use usvg::{self, Color, NodeExt, PathData, PathSegment, XmlOptions};

type Index = u32;
pub fn iterate(path: PathData, width: f64) -> (Vec<Vertex>, Vec<Index>) {
    let mut vertices: Vec<Vertex> = vec![];
    let mut indices: Vec<Index> = vec![];
    let mut current_xy: DVec2 = DVec2::new(0.0, 0.0);

    path.iter().for_each(|path| match path {
        PathSegment::MoveTo { x, y } => {
            current_xy = DVec2::new(*x, *y);
        }
        PathSegment::LineTo { x, y } => {
            // Below wiki is a reference to what is being done here
            // https://github.com/nical/lyon/wiki/Stroke-tessellation
            let next_xy = DVec2::new(*x, *y);
            let ((p0, p1), (p2, p3)) = line_to_parallel_lines((current_xy, next_xy), width);
            let new_vertices: Vec<Vertex> = [p0, p1, p2, p3]
                .iter()
                .map(|p| Vertex::from_vec2(p))
                .collect();
            let len = vertices.len() as u32;
            // indices pattern to create two triangles that make a rectangle
            let new_indices: Vec<Index> = [4, 3, 2, 3, 2, 1]
                .iter()
                .map(|index_diff| len - index_diff)
                .collect();
            vertices.extend(new_vertices);
            indices.extend(new_indices);
        }
        PathSegment::CurveTo {
            x1,
            y1,
            x2,
            y2,
            x,
            y,
        } => {}
        PathSegment::ClosePath => todo!(),
    });
    return (todo!(), todo!());
}
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct Vertex {
    position: [f32; 3],
    color: [f32; 3],
}
impl Vertex {
    fn from_vec2(v: &DVec2) -> Self {
        Self {
            position: [v.x as f32, v.y as f32, 0.0],
            ..Default::default()
        }
    }
}