//! The poker clients whose running locks every advice feature of Ringside.

use ringside_core::model::Site;

/// Executable names of the GGPoker client (lower case), as Windows reports them. `GGnet.exe` is
/// the table client the launcher starts; the others are names of earlier installers and skins.
const GG_EXES: &[&str] = &["ggnet.exe", "ggpoker.exe", "gglauncher.exe", "ggplauncher.exe"];
/// Folders GGPoker installs into; its launcher has only the generic name `launcher.exe`.
const GG_FOLDERS: &[&str] = &["ggpoker", "ggpcom", "ggnet"];

/// The client an executable belongs to, from its full path ("…\\PokerStars.exe", "…\\GGnet.exe").
pub fn client_of_exe(path: &str) -> Option<Site> {
    let path = path.to_lowercase();
    let exe = path.rsplit(['\\', '/']).next().unwrap_or_default();
    let in_gg_folder = || path.split(['\\', '/']).any(|dir| GG_FOLDERS.iter().any(|f| dir.starts_with(f)));
    if exe.starts_with("pokerstars") {
        Some(Site::PokerStars)
    } else if GG_EXES.contains(&exe) || (exe == "launcher.exe" && in_gg_folder()) {
        Some(Site::GgPoker)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_clients_by_their_executables() {
        for exe in [r"C:\Program Files\PokerStars.EU\PokerStars.exe", "PokerStarsUpdate.exe", "pokerstars.exe"] {
            assert_eq!(client_of_exe(exe), Some(Site::PokerStars), "{exe}");
        }
        for exe in [
            r"C:\Program Files\GGPoker UK\bin\GGnet.exe",
            r"C:\Users\du\AppData\Roaming\GGPCOM\bin\launcher.exe",
            "GGPoker.exe",
            "GGPLauncher.exe",
        ] {
            assert_eq!(client_of_exe(exe), Some(Site::GgPoker), "{exe}");
        }
        for exe in ["ringside.exe", "explorer.exe", "ggnet.exe.bak", "notggnet.exe", r"C:\Games\Steam\launcher.exe"] {
            assert_eq!(client_of_exe(exe), None, "{exe}");
        }
    }
}
