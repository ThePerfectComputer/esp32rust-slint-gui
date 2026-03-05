use alloc::boxed::Box;
use alloc::rc::Rc;
use core::cell::RefCell;
use esp_hal::time::Instant;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};

pub struct BackendState {
    pub window: RefCell<Option<Rc<MinimalSoftwareWindow>>>,
}

struct EspBackend {
    state: Rc<BackendState>,
}

impl slint::platform::Platform for EspBackend {
    fn create_window_adapter(
        &self,
    ) -> Result<Rc<dyn slint::platform::WindowAdapter>, slint::PlatformError> {
        let window = MinimalSoftwareWindow::new(RepaintBufferType::ReusedBuffer);
        self.state.window.replace(Some(window.clone()));
        Ok(window)
    }

    fn duration_since_start(&self) -> core::time::Duration {
        core::time::Duration::from_millis(Instant::now().duration_since_epoch().as_millis())
    }

    fn run_event_loop(&self) -> Result<(), slint::PlatformError> {
        panic!("run_event_loop is unused in this async embassy integration")
    }
}

pub fn install() -> Rc<BackendState> {
    let state = Rc::new(BackendState {
        window: RefCell::new(None),
    });
    slint::platform::set_platform(Box::new(EspBackend {
        state: state.clone(),
    }))
    .expect("Failed to set Slint platform");
    state
}
