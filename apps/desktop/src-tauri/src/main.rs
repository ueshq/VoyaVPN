// Release GUI builds must not open a console window on Windows. Child
// processes get `CREATE_NO_WINDOW` from `voya_platform::process`, so hiding the
// shell's own console no longer costs us any subprocess output visibility.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    voyavpn_lib::run();
}
