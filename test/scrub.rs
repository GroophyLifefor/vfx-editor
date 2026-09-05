use crate::{x_to_frame, TimeView};

#[test]
fn maps_edges_and_mid() {
    let v = TimeView {
        start: 0.0,
        span: 10.0,
    };
    assert_eq!(x_to_frame(0.0, 0.0, 100.0, v, 11), 0);
    assert_eq!(x_to_frame(100.0, 0.0, 100.0, v, 11), 10);
    assert_eq!(x_to_frame(50.0, 0.0, 100.0, v, 11), 5);
}
