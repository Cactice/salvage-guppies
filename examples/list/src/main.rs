use experiment::{responsive::layout_machine::LayoutMachine, scroll::ScrollState, uses::use_svg};
use guppies::bytemuck::cast_slice;
use guppies::{GpuRedraw, Guppy};
use mobile_entry_point::mobile_entry_point;

struct ListItem {
    word: String,
    icon: String,
}

pub fn main() {
    let mut layout_machine = LayoutMachine::default();

    let svg_set = use_svg(
        include_str!("../Left.svg").to_string(),
        |node, mut pass_down| {
            layout_machine.add_node(&node, &mut pass_down);
            (node, pass_down)
        },
        None,
    );

    let list = use_svg(
        include_str!("../Left.svg").to_string(),
        |node, mut pass_down| {
            layout_machine.add_node(&node, &mut pass_down);
            (node, pass_down)
        },
        Some("ListItem".to_string()),
    );

    let mut guppy = Guppy::new([GpuRedraw::default()]);

    guppy.register(move |event, gpu_redraws| {
        layout_machine.event_handler(event);
        gpu_redraws[0].update_texture([cast_slice(&layout_machine.transforms[..])].concat());
        gpu_redraws[0].update_triangles(
            svg_set
                .get_combined_geometries()
                // .extend(&list.get_combined_geometries())
                .triangles,
            0,
        );
    });

    guppy.start();
}

#[mobile_entry_point]
pub fn mobile_main() {
    main()
}