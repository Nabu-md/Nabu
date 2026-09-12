const DOCK_ICON_VARIANTS: [&[u8]; 10] = [
    include_bytes!("../resources/dock-icons/variant-1.png"),
    include_bytes!("../resources/dock-icons/variant-2.png"),
    include_bytes!("../resources/dock-icons/variant-3.png"),
    include_bytes!("../resources/dock-icons/variant-4.png"),
    include_bytes!("../resources/dock-icons/variant-5.png"),
    include_bytes!("../resources/dock-icons/variant-6.png"),
    include_bytes!("../resources/dock-icons/variant-7.png"),
    include_bytes!("../resources/dock-icons/variant-8.png"),
    include_bytes!("../resources/dock-icons/variant-9.png"),
    include_bytes!("../resources/dock-icons/variant-10.png"),
];

#[cfg(target_os = "macos")]
use objc2::MainThreadMarker;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AppIconMode {
    Variant1,
    Variant2,
    Variant3,
    Variant4,
    Variant5,
    Variant6,
    Variant7,
    Variant8,
    Variant9,
    Variant10,
}

impl AppIconMode {
    fn parse(value: &str) -> Result<Self, String> {
        match value {
            "variant-1" => Ok(Self::Variant1),
            "variant-2" => Ok(Self::Variant2),
            "variant-3" => Ok(Self::Variant3),
            "variant-4" => Ok(Self::Variant4),
            "variant-5" => Ok(Self::Variant5),
            "variant-6" => Ok(Self::Variant6),
            "variant-7" => Ok(Self::Variant7),
            "variant-8" => Ok(Self::Variant8),
            "variant-9" => Ok(Self::Variant9),
            "variant-10" => Ok(Self::Variant10),
            "light" => Ok(Self::Variant6),
            "dark" => Ok(Self::Variant1),
            _ => Err(format!("Unsupported app icon mode: {value}")),
        }
    }

    fn is_dark(self) -> bool {
        matches!(
            self,
            Self::Variant1
                | Self::Variant2
                | Self::Variant3
                | Self::Variant4
                | Self::Variant5
        )
    }

    fn png_bytes(self) -> &'static [u8] {
        match self {
            Self::Variant1 => DOCK_ICON_VARIANTS[0],
            Self::Variant2 => DOCK_ICON_VARIANTS[1],
            Self::Variant3 => DOCK_ICON_VARIANTS[2],
            Self::Variant4 => DOCK_ICON_VARIANTS[3],
            Self::Variant5 => DOCK_ICON_VARIANTS[4],
            Self::Variant6 => DOCK_ICON_VARIANTS[5],
            Self::Variant7 => DOCK_ICON_VARIANTS[6],
            Self::Variant8 => DOCK_ICON_VARIANTS[7],
            Self::Variant9 => DOCK_ICON_VARIANTS[8],
            Self::Variant10 => DOCK_ICON_VARIANTS[9],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Rgb {
    red: u8,
    green: u8,
    blue: u8,
}

impl Rgb {
    const fn new(red: u8, green: u8, blue: u8) -> Self {
        Self { red, green, blue }
    }

    fn for_vault_color(value: Option<&str>, mode: AppIconMode) -> Self {
        let [red, green, blue] =
            crate::workspace_colors::app_icon_rgb(value, mode.is_dark());
        Self::new(red, green, blue)
    }
}

fn is_droplet_pixel(pixel: &[u8]) -> bool {
    let [red, green, blue, alpha] = pixel else {
        return false;
    };
    *alpha > 0
        && *blue > 100
        && u16::from(*blue) * 4 > u16::from(*red) * 5
        && u16::from(*blue) * 6 > u16::from(*green) * 7
}

fn tint_channel(channel: u8, brightness: u8) -> u8 {
    let base = u16::from(channel);
    let value = u16::from(brightness);
    ((base * value + 127) / 255).min(255) as u8
}

fn recolor_icon(image: tauri::image::Image<'_>, accent: Rgb) -> tauri::image::Image<'static> {
    let width = image.width();
    let height = image.height();
    let mut rgba = image.rgba().to_vec();

    for pixel in rgba.chunks_exact_mut(4) {
        if !is_droplet_pixel(pixel) {
            continue;
        }
        let brightness = pixel[0].max(pixel[1]).max(pixel[2]);
        pixel[0] = tint_channel(accent.red, brightness);
        pixel[1] = tint_channel(accent.green, brightness);
        pixel[2] = tint_channel(accent.blue, brightness);
    }

    tauri::image::Image::new_owned(rgba, width, height)
}

fn active_vault_color(list: &crate::vault_list::VaultList) -> Option<&str> {
    let active_path = list.active_vault.as_deref()?;
    list.vaults
        .iter()
        .find(|vault| vault.path == active_path)
        .and_then(|vault| vault.color.as_deref())
}

fn vault_accent_color(mode: AppIconMode) -> Rgb {
    let launch_color =
        crate::vault_instance::current_launch().and_then(|launch| launch.vault_color);
    let registry = crate::vault_list::load_vault_list().ok();
    let vault_color = launch_color
        .as_deref()
        .or_else(|| registry.as_ref().and_then(active_vault_color));
    Rgb::for_vault_color(vault_color, mode)
}

#[cfg(target_os = "macos")]
objc2::extern_class!(
    #[unsafe(super(objc2_app_kit::NSResponder, objc2_foundation::NSObject))]
    #[name = "NSApplication"]
    struct NabuApplication;
);

#[cfg(target_os = "macos")]
impl NabuApplication {
    objc2::extern_methods!(
        #[unsafe(method(sharedApplication))]
        #[unsafe(method_family = none)]
        fn shared_application(marker: MainThreadMarker) -> objc2::rc::Retained<NabuApplication>;

        #[unsafe(method(setApplicationIconImage:))]
        #[unsafe(method_family = none)]
        fn set_nabu_application_icon(&self, image: &objc2_app_kit::NSImage);
    );
}

#[cfg(target_os = "macos")]
fn encode_png(image: &tauri::image::Image<'_>) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    let mut encoder = png::Encoder::new(&mut bytes, image.width(), image.height());
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder
        .write_header()
        .map_err(|error| format!("Failed to encode macOS app icon: {error}"))?;
    writer
        .write_image_data(image.rgba())
        .map_err(|error| format!("Failed to encode macOS app icon: {error}"))?;
    drop(writer);
    Ok(bytes)
}

#[cfg(target_os = "macos")]
fn set_native_app_icon(image: tauri::image::Image<'static>) -> Result<(), String> {
    use objc2::AllocAnyThread;
    use objc2_app_kit::NSImage;
    use objc2_foundation::NSData;

    let marker = MainThreadMarker::new()
        .ok_or_else(|| "App icon update must run on the main thread".to_string())?;
    let data = NSData::from_vec(encode_png(&image)?);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| "Failed to create macOS app icon image".to_string())?;
    let app = NabuApplication::shared_application(marker);
    app.set_nabu_application_icon(&image);
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn set_window_icons(
    app_handle: &tauri::AppHandle,
    image: tauri::image::Image<'static>,
) -> Result<(), String> {
    use tauri::Manager;

    for window in app_handle.webview_windows().into_values() {
        window
            .set_icon(image.clone())
            .map_err(|error| format!("Failed to update window icon: {error}"))?;
    }
    Ok(())
}

pub fn update_app_icon_for_theme(
    app_handle: &tauri::AppHandle,
    theme_mode: &str,
) -> Result<(), String> {
    let mode = AppIconMode::parse(theme_mode)?;
    let source = tauri::image::Image::from_bytes(mode.png_bytes())
        .map_err(|error| format!("Failed to decode app icon: {error}"))?;
    let image = recolor_icon(source, vault_accent_color(mode));

    #[cfg(target_os = "macos")]
    {
        app_handle
            .run_on_main_thread(move || {
                if let Err(error) = set_native_app_icon(image) {
                    log::warn!("{error}");
                }
            })
            .map_err(|error| format!("Failed to schedule app icon update: {error}"))?;
    }

    #[cfg(not(target_os = "macos"))]
    set_window_icons(app_handle, image)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{active_vault_color, is_droplet_pixel, recolor_icon, AppIconMode, Rgb};
    use crate::vault_list::{VaultEntry, VaultList};

    #[test]
    fn parses_supported_icon_modes() {
        assert_eq!(AppIconMode::parse("light"), Ok(AppIconMode::Variant6));
        assert_eq!(AppIconMode::parse("dark"), Ok(AppIconMode::Variant1));
        assert_eq!(AppIconMode::parse("variant-1"), Ok(AppIconMode::Variant1));
        assert_eq!(AppIconMode::parse("variant-10"), Ok(AppIconMode::Variant10));
    }

    #[test]
    fn rejects_unknown_icon_modes() {
        assert!(AppIconMode::parse("system").is_err());
    }

    #[test]
    fn maps_vault_colors_to_light_and_dark_interface_accents() {
        assert_eq!(
            Rgb::for_vault_color(Some("red"), AppIconMode::Variant6),
            Rgb::new(229, 62, 62)
        );
        assert_eq!(
            Rgb::for_vault_color(Some("red"), AppIconMode::Variant1),
            Rgb::new(255, 138, 134)
        );
        assert_eq!(
            Rgb::for_vault_color(None, AppIconMode::Variant6),
            Rgb::new(21, 93, 255)
        );
        assert_eq!(
            Rgb::for_vault_color(Some("pink"), AppIconMode::Variant1),
            Rgb::new(120, 164, 255)
        );
    }

    #[test]
    fn resolves_the_active_vault_color_from_the_registry() {
        let list = VaultList {
            vaults: vec![VaultEntry {
                label: "Laputa".to_string(),
                path: "/vaults/laputa".to_string(),
                color: Some("red".to_string()),
                ..Default::default()
            }],
            active_vault: Some("/vaults/laputa".to_string()),
            ..Default::default()
        };

        assert_eq!(active_vault_color(&list), Some("red"));
    }

    #[test]
    fn recolors_blue_droplet_pixels_without_touching_neutral_background_pixels() {
        let source = tauri::image::Image::new(&[21, 93, 255, 255, 21, 25, 35, 255], 2, 1);
        let recolored = recolor_icon(source, Rgb::new(229, 62, 62));

        assert_eq!(&recolored.rgba()[..4], &[229, 62, 62, 255]);
        assert_eq!(&recolored.rgba()[4..], &[21, 25, 35, 255]);
        assert!(is_droplet_pixel(&[21, 93, 255, 255]));
        assert!(!is_droplet_pixel(&[21, 25, 35, 255]));
    }
}
