#![no_std]
extern crate alloc;

#[cfg(target_arch = "xtensa")]
pub mod embedded;

slint::slint! {
    export component DemoApp inherits Window {
        title: "ESP Slint Demo";
        width: 320px;
        height: 240px;
        background: #000000;

        in-out property <bool> left_active: false;
        in-out property <bool> right_active: false;

        callback left_toggle_requested();
        callback right_toggle_requested();

        Rectangle {
            x: 0px;
            y: 0px;
            width: parent.width / 2;
            height: parent.height;
            background: root.left_active ? rgb(46, 204, 113) : rgb(128, 41, 55);
            TouchArea {
                clicked => {
                    root.left_toggle_requested();
                }
            }
        }

        Rectangle {
            x: parent.width / 2;
            y: 0px;
            width: parent.width / 2;
            height: parent.height;
            background: root.right_active ? rgb(230, 126, 34) : rgb(17, 24, 128);
            TouchArea {
                clicked => {
                    root.right_toggle_requested();
                }
            }
        }

        Rectangle {
            x: parent.width / 2 - 1px;
            y: 0px;
            width: 2px;
            height: parent.height;
            background: rgb(51, 65, 85);
        }
    }
}

pub fn install_demo_logic(app: &DemoApp) {
    let left_weak = app.as_weak();
    app.on_left_toggle_requested(move || {
        if let Some(app) = left_weak.upgrade() {
            app.set_left_active(!app.get_left_active());
        }
    });

    let right_weak = app.as_weak();
    app.on_right_toggle_requested(move || {
        if let Some(app) = right_weak.upgrade() {
            app.set_right_active(!app.get_right_active());
        }
    });
}
