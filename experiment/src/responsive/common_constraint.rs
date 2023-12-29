use super::constraint::{XConstraint, YConstraint};
use guppies::glam::{Mat4, Vec3};

pub(crate) enum CommonConstraint {
    Start(f32),
    End(f32),
    StartAndEnd { start: f32, end: f32 },
    Center(f32),
    Scale,
}

impl From<XConstraint> for CommonConstraint {
    fn from(x_constraint: XConstraint) -> Self {
        match x_constraint {
            XConstraint::Left(left) => CommonConstraint::Start(left),
            XConstraint::Right(right) => CommonConstraint::End(right),
            XConstraint::LeftAndRight { left, right } => CommonConstraint::StartAndEnd {
                start: left,
                end: right,
            },
            XConstraint::Center(x) => CommonConstraint::Center(x),
            XConstraint::Scale => CommonConstraint::Scale,
        }
    }
}
impl From<YConstraint> for CommonConstraint {
    fn from(y_constraint: YConstraint) -> Self {
        match y_constraint {
            YConstraint::Top(top) => CommonConstraint::Start(top),
            YConstraint::Bottom(bottom) => CommonConstraint::End(bottom),
            YConstraint::TopAndBottom { top, bottom } => CommonConstraint::StartAndEnd {
                start: bottom,
                end: top,
            },
            YConstraint::Center(y) => CommonConstraint::Center(y),
            YConstraint::Scale => CommonConstraint::Scale,
        }
    }
}
impl CommonConstraint {
    pub(crate) fn to_transform<F: Fn(Vec3) -> f32, G: Fn(f32, f32) -> Vec3>(
        self,
        bbox: Mat4,
        parent_bbox: Mat4,
        accessor: F,
        composer: G,
    ) -> Mat4 {
        let fill = Mat4::from_scale(composer(
            accessor(parent_bbox.to_scale_rotation_translation().0)
                / accessor(bbox.to_scale_rotation_translation().0),
            1.0,
        ));

        let (left_align, right_align, center) = prepare_anchor_points(bbox, &accessor, &composer);
        let (left_align, right_align, center) = (
            left_align.inverse(),
            right_align.inverse(),
            center.inverse(),
        );
        let (parent_edge_left, parent_edge_right, parent_center) =
            prepare_anchor_points(parent_bbox, &accessor, &composer);

        match self {
            CommonConstraint::Start(left) => {
                parent_edge_left * left_align * Mat4::from_translation(composer(left, 0.))
            }
            CommonConstraint::End(right) => {
                parent_edge_right * right_align * Mat4::from_translation(composer(right, 0.))
            }
            CommonConstraint::Center(rightward_from_center) => {
                parent_center * center * Mat4::from_translation(composer(rightward_from_center, 0.))
            }
            CommonConstraint::StartAndEnd { start, end } => {
                todo!();
            }
            CommonConstraint::Scale => fill * center,
        }
    }
}

fn prepare_anchor_points<F: Fn(Vec3) -> f32, G: Fn(f32, f32) -> Vec3>(
    bbox: Mat4,
    accessor: &F,
    composer: &G,
) -> (Mat4, Mat4, Mat4) {
    let (bbox_scale, _, bbox_translation) = bbox.to_scale_rotation_translation();

    let start_align = Mat4::from_translation(composer(accessor(bbox_translation), 0.));
    let end_align = Mat4::from_translation(composer(
        accessor(bbox_translation) + accessor(bbox_scale),
        0.,
    ));
    let center = Mat4::from_translation(composer(
        accessor(bbox_translation) + accessor(bbox_scale) / 2.,
        0.,
    ));

    (start_align, end_align, center)
}
