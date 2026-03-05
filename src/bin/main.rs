#![no_std]
#![no_main]
#![deny(
    clippy::mem_forget,
    reason = "mem::forget is generally not safe to do with esp_hal types, especially those \
    holding buffers for the duration of a data transfer."
)]
#![deny(clippy::large_stack_frames)]

use embassy_executor::Spawner;
use embassy_time::{Duration, Timer};
use esp_hal::clock::CpuClock;
use esp_hal::timer::timg::TimerGroup;
use my_esp_project::embedded::board::split_board_resources;
use my_esp_project::embedded::ui_runtime::run_ui;
use rtt_target::rprintln;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    rprintln!("PANIC: {:?}", info);
    loop {}
}

esp_bootloader_esp_idf::esp_app_desc!();

#[esp_rtos::main]
async fn main(spawner: Spawner) -> ! {
    rtt_target::rtt_init_print!();

    let config = esp_hal::Config::default().with_cpu_clock(CpuClock::max());
    let peripherals = esp_hal::init(config);
    let board = split_board_resources(peripherals);
    esp_alloc::heap_allocator!(size: 128 * 1024);
    rprintln!("Heap initialized ({} bytes free)", esp_alloc::HEAP.free());

    let timg0 = TimerGroup::new(board.timg0);
    esp_rtos::start(timg0.timer0);

    let ui_task = run_ui(board.ui);
    spawner.spawn(ui_task).expect("Failed to spawn run_ui");

    loop {
        // Keep main alive while UI work runs in the spawned Embassy task.
        Timer::after(Duration::from_secs(1)).await;
    }
}
