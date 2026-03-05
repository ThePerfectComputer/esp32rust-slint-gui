#![no_std]
extern crate alloc;

use alloc::format;

#[cfg(target_arch = "xtensa")]
pub mod embedded;

slint::include_modules!();

pub fn install_demo_logic(app: &DemoApp) {
    const TRACKS: [(&str, &str); 5] = [
        ("Neon Skyline", "Code Runner"),
        ("Rusty Beats", "Zero Cost"),
        ("Heap of Dreams", "Borrow Checker"),
        ("Pointer Dance", "Unsafe Trio"),
        ("S3 Sunsets", "Embassy Wave"),
    ];

    app.set_song_title(TRACKS[0].0.into());
    app.set_artist_name(TRACKS[0].1.into());

    let toggle_weak = app.as_weak();
    app.on_toggle_playback(move || {
        if let Some(app) = toggle_weak.upgrade() {
            let playing = !app.get_is_playing();
            app.set_is_playing(playing);
            app.set_status_text(if playing { "Playback resumed" } else { "Playback paused" }.into());
        }
    });

    let previous_weak = app.as_weak();
    app.on_previous_track(move || {
        if let Some(app) = previous_weak.upgrade() {
            let current = app.get_song_index();
            let next = if current <= 0 {
                TRACKS.len() as i32 - 1
            } else {
                current - 1
            };
            let (title, artist) = TRACKS[next as usize];
            app.set_song_index(next);
            app.set_song_title(title.into());
            app.set_artist_name(artist.into());
            app.set_status_text(format!("Selected: {}", title).into());
        }
    });

    let next_weak = app.as_weak();
    app.on_next_track(move || {
        if let Some(app) = next_weak.upgrade() {
            let next = (app.get_song_index() + 1) % TRACKS.len() as i32;
            let (title, artist) = TRACKS[next as usize];
            app.set_song_index(next);
            app.set_song_title(title.into());
            app.set_artist_name(artist.into());
            app.set_status_text(format!("Selected: {}", title).into());
        }
    });

    let volume_up_weak = app.as_weak();
    app.on_volume_up(move || {
        if let Some(app) = volume_up_weak.upgrade() {
            let next = (app.get_volume() + 1).min(10);
            app.set_volume(next);
            app.set_status_text(format!("Volume: {}", next).into());
        }
    });

    let volume_down_weak = app.as_weak();
    app.on_volume_down(move || {
        if let Some(app) = volume_down_weak.upgrade() {
            let next = (app.get_volume() - 1).max(0);
            app.set_volume(next);
            app.set_status_text(format!("Volume: {}", next).into());
        }
    });

    let digit_weak = app.as_weak();
    app.on_digit_pressed(move |digit| {
        if let Some(app) = digit_weak.upgrade() {
            let current = app.get_dialed_number();
            if current.len() >= 16 || digit.is_empty() {
                return;
            }
            let mut next = format!("{}", current);
            next.push_str(digit.as_str());
            app.set_dialed_number(next.into());
        }
    });

    let backspace_weak = app.as_weak();
    app.on_backspace_requested(move || {
        if let Some(app) = backspace_weak.upgrade() {
            let mut value = format!("{}", app.get_dialed_number());
            value.pop();
            app.set_dialed_number(value.into());
        }
    });

    let clear_weak = app.as_weak();
    app.on_clear_requested(move || {
        if let Some(app) = clear_weak.upgrade() {
            app.set_dialed_number("".into());
            app.set_status_text("Dialer cleared".into());
        }
    });

    let call_weak = app.as_weak();
    app.on_call_requested(move || {
        if let Some(app) = call_weak.upgrade() {
            let number = app.get_dialed_number();
            if number.is_empty() {
                app.set_status_text("Enter a number before calling".into());
            } else {
                app.set_status_text(format!("Calling {}...", number).into());
            }
        }
    });

    let contact_weak = app.as_weak();
    app.on_contact_selected(move |index, name, number| {
        if let Some(app) = contact_weak.upgrade() {
            app.set_selected_contact(index);
            app.set_dialed_number(number.clone());
            app.set_status_text(format!("Loaded {}", name).into());
        }
    });
}
