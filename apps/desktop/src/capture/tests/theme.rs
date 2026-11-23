//! Inspect actual rendered input pixels in both palettes and narrow layouts.

use super::*;
use crate::binding;

pub(super) fn check(
    app: &AppWindow,
    window: &MinimalSoftwareWindow,
    pixels: &mut [slint::Rgb8Pixel],
) {
    for (width, height) in [(480, 600), (1160, 770)] {
        app.window().set_size(PhysicalSize::new(width, height));
        for appearance in [1, 2] {
            app.set_appearance(appearance);
            fixtures::apply(app, "preferences-presets");
            slint::platform::update_timers_and_animations();
            app.window().request_redraw();
            window.draw_if_needed(|renderer| {
                renderer.render(pixels, width as usize);
            });
            assert_eq!(app.get_dark_mode(), appearance == 2);
            let left = if width < 760 { 40 } else { 192 };
            let field = pixels[350 * width as usize + left + 200];
            let background = luminance(field);
            let ink: Vec<_> = (342..357)
                .flat_map(|y| (left + 14..left + 90).map(move |x| y * width as usize + x))
                .map(|offset| luminance(pixels[offset]))
                .collect();
            let text = if appearance == 2 {
                ink.into_iter().fold(0.0_f64, f64::max)
            } else {
                ink.into_iter().fold(1.0_f64, f64::min)
            };
            let ratio = (background.max(text) + 0.05) / (background.min(text) + 0.05);
            assert!(
                ratio >= 4.5,
                "Rendered input contrast must meet 4.5:1, got {ratio}"
            );
        }
    }
    app.set_appearance(1);
    app.window().set_size(PhysicalSize::new(1160, 770));
    fixtures::apply(app, "home-connected");
    assert_eq!(app.get_status(), "Connected · Japan");
    binding::apply(
        app,
        &vmate_engine::model::Snapshot {
            connected: true,
            ..Default::default()
        },
    );
    assert_eq!(app.get_status(), "Connected");
    app.set_message("Disconnecting and reaping owned processes…".into());
    app.set_busy(true);
    assert!(app.get_stopping());
    app.set_busy(false);
    assert!(
        !app.get_stopping(),
        "An idle app must never display Disconnecting"
    );
}

fn luminance(pixel: slint::Rgb8Pixel) -> f64 {
    let linear = |value: u8| {
        let value = f64::from(value) / 255.0;
        if value <= 0.04045 {
            value / 12.92
        } else {
            ((value + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(pixel.r) + 0.7152 * linear(pixel.g) + 0.0722 * linear(pixel.b)
}
