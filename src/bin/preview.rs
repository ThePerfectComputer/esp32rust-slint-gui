use my_esp_project::{DemoApp, install_demo_logic};
use slint::ComponentHandle;

fn main() {
    let app = DemoApp::new().expect("Failed to create preview app");
    install_demo_logic(&app);
    app.run().expect("Desktop preview failed");
}
