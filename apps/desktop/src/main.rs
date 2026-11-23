//! Slint presentation bound to the toolkit-independent application engine.

mod binding;
mod capture;
mod fixtures;
#[cfg(target_os = "linux")]
mod tray;
slint::include_modules!();

fn main() -> anyhow::Result<()> {
    let arguments: Vec<_> = std::env::args().collect();
    if let Some(index) = arguments.iter().position(|v| v == "--capture-dir") {
        return capture::run(
            arguments
                .get(index + 1)
                .ok_or_else(|| anyhow::anyhow!("provide capture directory"))?,
        );
    }
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(|attributes| {
            #[cfg(target_os = "linux")]
            {
                use slint::winit_030::winit::platform::{
                    wayland::WindowAttributesExtWayland, x11::WindowAttributesExtX11,
                };
                let attributes = WindowAttributesExtX11::with_name(attributes, "vmate", "vmate");
                WindowAttributesExtWayland::with_name(attributes, "vmate", "vmate")
            }
            #[cfg(not(target_os = "linux"))]
            {
                attributes
            }
        })
        .select()?;
    let engine = vmate_engine::Engine::start(vmate_engine::Configuration::system()?)?;
    let app = AppWindow::new()?;
    let _timer = binding::bind(&app, engine.port());
    app.show()?;
    slint::run_event_loop_until_quit()?;
    drop(engine);
    Ok(())
}
