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
mod tests;
