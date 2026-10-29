use std::fs;
use std::path::Path;

/// Only offer hardware-specific defaults when the corresponding device exists.
/// Modules already in a document remain editable, including imported laptop presets.
pub fn module_available(module: &str) -> bool {
    module_available_at(module, Path::new("/sys/class"))
}

fn module_available_at(module: &str, class: &Path) -> bool {
    match module {
        "battery" => fs::read_dir(class.join("power_supply"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| {
                let device = entry.path();
                fs::read_to_string(device.join("type")).is_ok_and(|kind| kind.trim() == "Battery")
                    && !fs::read_to_string(device.join("scope"))
                        .is_ok_and(|scope| scope.trim() == "Device")
            }),
        "backlight" => fs::read_dir(class.join("backlight"))
            .into_iter()
            .flatten()
            .filter_map(Result::ok)
            .any(|entry| entry.path().join("brightness").is_file()),
        _ => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_does_not_offer_laptop_hardware() {
        let class = tempfile::tempdir().unwrap();
        assert!(!module_available_at("battery", class.path()));
        assert!(!module_available_at("backlight", class.path()));
        assert!(module_available_at("cpu", class.path()));
        assert!(module_available_at("network", class.path()));
    }

    #[test]
    fn ac_and_peripheral_batteries_do_not_enable_system_battery() {
        let class = tempfile::tempdir().unwrap();
        let ac = class.path().join("power_supply/AC");
        let mouse = class.path().join("power_supply/mouse");
        fs::create_dir_all(&ac).unwrap();
        fs::write(ac.join("type"), "Mains\n").unwrap();
        fs::create_dir_all(&mouse).unwrap();
        fs::write(mouse.join("type"), "Battery\n").unwrap();
        fs::write(mouse.join("scope"), "Device\n").unwrap();
        assert!(!module_available_at("battery", class.path()));

        let battery = class.path().join("power_supply/BAT0");
        fs::create_dir_all(&battery).unwrap();
        fs::write(battery.join("type"), "Battery\n").unwrap();
        assert!(module_available_at("battery", class.path()));
        fs::write(battery.join("scope"), "System\n").unwrap();
        assert!(module_available_at("battery", class.path()));
    }

    #[test]
    fn laptop_backlight_is_available() {
        let class = tempfile::tempdir().unwrap();
        let panel = class.path().join("backlight/intel_backlight");
        fs::create_dir_all(&panel).unwrap();
        fs::write(panel.join("brightness"), "50\n").unwrap();
        assert!(module_available_at("backlight", class.path()));
    }
}
