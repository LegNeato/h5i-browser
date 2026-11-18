use super::*;

#[test]
fn scaled_capture_keeps_the_same_viewport_content() {
    let mut page = page_from(
        "<style>html,body{margin:0}body{width:400px;height:800px;background:red}div{position:absolute;left:200px;top:100px;width:200px;height:100px;background:blue}footer{position:absolute;top:400px;width:400px;height:100px;background:green}</style><div></div><footer></footer>",
        Policy::new(),
        Arc::new(MemorySink::new()),
    );
    let capture = page.screenshot_png_scaled(200).unwrap();
    let image = decoded(&capture.png);
    assert_eq!((capture.width, capture.height), (200, 100));
    assert_eq!(image.dimensions(), (200, 100));
    assert_eq!(image.get_pixel(25, 25).0, [255, 0, 0]);
    assert_eq!(image.get_pixel(175, 75).0, [0, 0, 255]);
    assert!(!image.pixels().any(|pixel| pixel.0 == [0, 128, 0]));
}

#[test]
fn scaled_capture_preserves_scroll_layout_and_observations() {
    let mut page = page_from(
        "<style>html,body{margin:0}body{height:800px;background:red}div{position:absolute;top:400px;height:200px;width:400px;background:blue}</style><div></div>",
        Policy::new(),
        Arc::new(MemorySink::new()),
    );
    assert!(page.scroll_by(0.0, 400.0));
    let scroll = page.scroll_offset();
    let before = page.screenshot_png().unwrap();
    let snapshot = page.snapshot().render();
    let capture = page.screenshot_png_scaled(200).unwrap();
    assert_eq!((capture.width, capture.height), (200, 100));
    let image = decoded(&capture.png);
    assert_eq!(image.get_pixel(100, 50).0, [0, 0, 255]);
    assert_eq!(page.scroll_offset(), scroll);
    assert_eq!(page.snapshot().render(), snapshot);
    assert_eq!(page.screenshot_png().unwrap(), before);
    assert!(page.screenshot_png_scaled(0).is_err());
    assert_eq!(page.scroll_offset(), scroll);
}
