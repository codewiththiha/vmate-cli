//! Deterministic real-component screenshots without display or network access.

use crate::{AppWindow, fixtures};
use slint::platform::{
    Platform, WindowAdapter,
    software_renderer::{MinimalSoftwareWindow, RepaintBufferType},
};
use slint::{ComponentHandle, PhysicalSize};
use std::{path::Path, rc::Rc};

struct Headless(Rc<MinimalSoftwareWindow>);
impl Platform for Headless {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, slint::PlatformError> {
        Ok(self.0.clone())
    }
}

pub(super) fn run(directory: &str) -> anyhow::Result<()> {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(Headless(window.clone())))?;
    std::fs::create_dir_all(directory)?;
    let app = AppWindow::new()?;
    app.set_fixture(true);
    app.set_reduced(true);
    app.show()?;
    for (width, height, suffix) in [
        (480, 600, "minimum"),
        (860, 740, "compact"),
        (1160, 770, "desktop"),
        (1440, 900, "wide"),
    ] {
        app.window().set_size(PhysicalSize::new(width, height));
        for scenario in fixtures::SCENARIOS {
            fixtures::apply(&app, scenario);
            slint::platform::update_timers_and_animations();
            app.window().request_redraw();
            let mut pixels = vec![slint::Rgb8Pixel::default(); (width * height) as usize];
            window.draw_if_needed(|renderer| {
                renderer.render(&mut pixels, width as usize);
            });
            let bytes: Vec<u8> = pixels.iter().flat_map(|p| [p.r, p.g, p.b]).collect();
            let path = Path::new(directory).join(format!("{scenario}-{suffix}.png"));
            image::save_buffer(path, &bytes, width, height, image::ColorType::Rgb8)?;
        }
    }
    app.hide()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    mod flow;
    use super::*;
    use slint::{
        LogicalPosition,
        platform::{PointerEventButton, WindowEvent},
    };

    #[test]
    fn actual_component_renders_and_accepts_pointer_navigation() {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
        slint::platform::set_platform(Box::new(Headless(window.clone()))).unwrap();
        let app = AppWindow::new().unwrap();
        app.window().set_size(PhysicalSize::new(1160, 770));
        app.set_fixture(true);
        fixtures::apply(&app, "library");
        app.show().unwrap();
        slint::platform::update_timers_and_animations();
        let mut pixels = vec![slint::Rgb8Pixel::default(); 1160 * 770];
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 1160);
        });
        assert!(pixels.windows(2).any(|pair| pair[0] != pair[1]));
        flow::brand_alignment(&pixels);
        let clicked = Rc::new(std::cell::Cell::new(false));
        let flag = clicked.clone();
        app.on_scan(move || flag.set(true));
        let click = |x, y| {
            app.window().dispatch_event(WindowEvent::PointerMoved {
                position: LogicalPosition::new(x, y),
            });
            app.window().dispatch_event(WindowEvent::PointerPressed {
                position: LogicalPosition::new(x, y),
                button: PointerEventButton::Left,
            });
            app.window().dispatch_event(WindowEvent::PointerReleased {
                position: LogicalPosition::new(x, y),
                button: PointerEventButton::Left,
            });
        };
        click(1080.0, 165.0);
        assert!(
            clicked.get(),
            "primary scan control must accept pointer activation"
        );
        click(80.0, 323.0);
        assert_eq!(app.get_page(), 1, "sidebar must navigate to Connections");
        fixtures::apply(&app, "connected-library");
        clicked.set(false);
        app.window().request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 1160);
        });
        click(1080.0, 165.0);
        assert!(
            !clicked.get(),
            "library scan must stay disabled during a VPN connection"
        );
        home_controls(&app, &window, &mut pixels, &click);
        flow::check(&app, &window, &mut pixels, &click);
        app.hide().unwrap();
    }

    fn home_controls(
        app: &AppWindow,
        window: &MinimalSoftwareWindow,
        pixels: &mut [slint::Rgb8Pixel],
        click: &dyn Fn(f32, f32),
    ) {
        let connects = Rc::new(std::cell::Cell::new(0));
        let cancels = Rc::new(std::cell::Cell::new(0));
        let flag = connects.clone();
        app.on_auto_connect(move || flag.set(flag.get() + 1));
        let flag = cancels.clone();
        app.on_cancel(move || flag.set(flag.get() + 1));
        for scenario in [
            "home-idle",
            "home-scanning",
            "home-connected",
            "home-stopping",
        ] {
            fixtures::apply(app, scenario);
            app.window().request_redraw();
            window.draw_if_needed(|renderer| {
                renderer.render(pixels, 1160);
            });
            click(660.0, 400.0);
        }
        assert_eq!(connects.get(), 1, "idle Home starts one automatic workflow");
        assert_eq!(
            cancels.get(),
            2,
            "busy and connected Home cancel, stopping cannot restart"
        );
    }
}
