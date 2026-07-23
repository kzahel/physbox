#![forbid(unsafe_code)]

use physbox_engine::EngineConfig;
use physbox_launch::{LaunchHost, parse_os_args, usage};
use physbox_shell::{FrameMarkerGate, WinitFrameDriver};
use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::window::{WindowAttributes, WindowId};

fn main() {
    let request = match parse_os_args(LaunchHost::Desktop, std::env::args_os()) {
        Ok(request) => request,
        Err(error) => {
            eprintln!(
                "{error}\n\n{}",
                usage("physbox-desktop", LaunchHost::Desktop)
            );
            std::process::exit(2);
        }
    };
    if request.help_requested {
        println!("{}", usage("physbox-desktop", LaunchHost::Desktop));
        return;
    }
    let engine_config = match EngineConfig::try_from(&request.options) {
        Ok(config) => config,
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(2);
        }
    };
    let viewport = [request.options.width, request.options.height];
    let event_loop = EventLoop::new().expect("failed to create desktop event loop");
    let mut app = DesktopApp {
        viewport,
        driver: WinitFrameDriver::new(engine_config),
        frame_markers: FrameMarkerGate::new(request.options.frame_markers),
    };
    event_loop
        .run_app(&mut app)
        .expect("PhysBox 5 desktop event loop failed");
}

struct DesktopApp {
    viewport: [u32; 2],
    driver: WinitFrameDriver,
    frame_markers: FrameMarkerGate,
}

impl ApplicationHandler for DesktopApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let attributes = WindowAttributes::default()
            .with_title("PhysBox 5 — initializing WebGPU")
            .with_inner_size(LogicalSize::new(self.viewport[0], self.viewport[1]));
        match self.driver.resume(event_loop, attributes) {
            Ok(Some(adapter)) => {
                println!("PHYSBOX5_MARK_ADAPTER {}", adapter.marker_json());
                self.driver
                    .set_window_title(&format!("PhysBox 5 — {}", adapter.name));
            }
            Ok(None) => {}
            Err(error) => fatal(event_loop, error),
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        self.driver.suspend();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: WindowId,
        event: WindowEvent,
    ) {
        match self.driver.handle_window_event(window_id, event) {
            Ok(outcome) => {
                if let Some(report) = outcome.report
                    && self.frame_markers.should_emit(&report)
                {
                    println!("PHYSBOX5_MARK_FRAME_RENDERED {}", report.marker_json());
                }
                if outcome.exit_requested {
                    event_loop.exit();
                }
            }
            Err(error) => fatal(event_loop, error),
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.driver.about_to_wait(event_loop);
    }
}

fn fatal(event_loop: &ActiveEventLoop, error: String) {
    eprintln!("PhysBox 5 could not start: {error}");
    event_loop.exit();
}
