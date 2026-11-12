use gpui::{Hsla, PathBuilder, Pixels, Point, Window, point, px};

pub(super) fn paint_strokes(
    window: &mut Window,
    strokes: &[Vec<[f32; 2]>],
    place: impl Fn([f32; 2]) -> Point<Pixels>,
    width: Pixels,
    color: Hsla,
) {
    for stroke in strokes {
        let Some(first) = stroke.first() else {
            continue;
        };
        let start = place(*first);
        let mut builder = PathBuilder::stroke(width);
        builder.move_to(start);
        if stroke.len() == 1 {
            builder.line_to(start + point(width / 2.0, px(0.0)));
        }
        for position in &stroke[1..] {
            builder.line_to(place(*position));
        }
        if let Ok(path) = builder.build() {
            window.paint_path(path, color);
        }
    }
}
