fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/mac_reminders.m")
            .flag("-fobjc-arc")
            .compile("fox_reminders");
        println!("cargo:rerun-if-changed=src/mac_reminders.m");
        println!("cargo:rustc-link-lib=framework=UserNotifications");
        println!("cargo:rustc-link-lib=framework=Foundation");
    }
    tauri_build::build()
}
