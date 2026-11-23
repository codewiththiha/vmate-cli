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
    for (width, height, suffix) in [(1160, 770, "desktop"), (860, 740, "compact")] {
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
        click(1050.0, 168.0);
        assert!(
            clicked.get(),
            "primary scan control must accept pointer activation"
        );
        click(80.0, 280.0);
        assert_eq!(app.get_page(), 1, "sidebar must navigate to Connections");
        fixtures::apply(&app, "connected-library");
        clicked.set(false);
        app.window().request_redraw();
        window.draw_if_needed(|renderer| {
            renderer.render(&mut pixels, 1160);
        });
        click(1050.0, 168.0);
        assert!(
            !clicked.get(),
            "library scan must stay disabled during a VPN connection"
        );
        app.hide().unwrap();
    }
}
