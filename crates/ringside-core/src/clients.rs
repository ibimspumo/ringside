//! The poker clients whose running locks every advice feature of Ringside.

use std::process::Command;

use crate::model::Site;

/// Executable names of the GGPoker client (lower case, without ".exe"). `GGnet` is the table
/// client the launcher starts; the others are names of earlier installers and skins.
const GG_EXES: &[&str] = &["ggnet", "ggpoker", "gglauncher", "ggplauncher"];
/// Folders GGPoker installs into; its Windows launcher has only the generic name `launcher.exe`,
/// on macOS every process of its app bundle counts.
const GG_FOLDERS: &[&str] = &["ggpoker", "ggpcom", "ggnet"];

/// The client an executable belongs to, from its full path ("…\\PokerStars.exe", "…\\GGnet.exe",
/// "/Applications/PokerStars.app/Contents/MacOS/PokerStars").
pub fn client_of_exe(path: &str) -> Option<Site> {
    let path = path.trim().to_lowercase();
    let exe = path.rsplit(['\\', '/']).next().unwrap_or_default();
    let stem = exe.strip_suffix(".exe").unwrap_or(exe);
    let gg_dir = |dir: &str| GG_FOLDERS.iter().any(|f| dir.starts_with(f));
    let in_gg_folder = || path.split(['\\', '/']).any(gg_dir);
    let in_gg_bundle = || path.split('/').any(|dir| dir.ends_with(".app") && gg_dir(dir)) && path.contains("/contents/");
    if stem.starts_with("pokerstars") {
        Some(Site::PokerStars)
    } else if GG_EXES.contains(&stem) || (stem == "launcher" && in_gg_folder()) || in_gg_bundle() {
        Some(Site::GgPoker)
    } else {
        None
    }
}

/// The poker client that runs, PokerStars before GGPoker, from the process list.
pub fn running_client() -> Result<Option<Site>, String> {
    let clients: Vec<Site> = process_paths()?.iter().filter_map(|p| client_of_exe(p)).collect();
    Ok(clients.iter().copied().find(|s| *s == Site::PokerStars).or(clients.first().copied()))
}

#[cfg(windows)]
fn process_paths() -> Result<Vec<String>, String> {
    let out = Command::new("tasklist").args(["/FO", "CSV", "/NH"]).output().map_err(|e| format!("tasklist: {e}"))?;
    if !out.status.success() {
        return Err(format!("tasklist failed ({})", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split("\",\"").next().map(|n| n.trim_matches('"').to_string()))
        .collect())
}

#[cfg(not(windows))]
fn process_paths() -> Result<Vec<String>, String> {
    let out = Command::new("ps").args(["-A", "-o", "comm="]).output().map_err(|e| format!("ps: {e}"))?;
    if !out.status.success() {
        return Err(format!("ps failed ({})", out.status));
    }
    Ok(String::from_utf8_lossy(&out.stdout).lines().map(str::to_string).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn knows_the_clients_by_their_executables() {
        for exe in [
            r"C:\Program Files\PokerStars.EU\PokerStars.exe",
            r"C:\Program Files\PokerStars.DE\PokerStarsDE.exe",
            "PokerStarsUpdate.exe",
            "pokerstars.exe",
            "/Applications/PokerStars.app/Contents/MacOS/PokerStars",
            "/Applications/PokerStarsEU.app/Contents/MacOS/PokerStarsEU",
        ] {
            assert_eq!(client_of_exe(exe), Some(Site::PokerStars), "{exe}");
        }
        for exe in [
            r"C:\Program Files\GGPoker UK\bin\GGnet.exe",
            r"C:\Users\du\AppData\Roaming\GGPCOM\bin\launcher.exe",
            "GGPoker.exe",
            "GGPLauncher.exe",
            "/Applications/GGPoker.app/Contents/MacOS/GGPoker",
            "/Applications/GGPoker.app/Contents/Frameworks/GGnet.app/Contents/MacOS/GGnet",
            "/Applications/GGPoker.app/Contents/MacOS/launcher",
        ] {
            assert_eq!(client_of_exe(exe), Some(Site::GgPoker), "{exe}");
        }
        for exe in [
            "ringside.exe",
            "explorer.exe",
            "ggnet.exe.bak",
            "notggnet.exe",
            r"C:\Games\Steam\launcher.exe",
            "/Applications/PokerStars.app/Contents/MacOS/Helper",
            "/Applications/Ringside.app/Contents/MacOS/ringside",
            "/Users/du/ggpoker-hands/notes",
            "stars",
        ] {
            assert_eq!(client_of_exe(exe), None, "{exe}");
        }
    }

    #[test]
    fn reads_the_process_list() {
        assert!(!process_paths().unwrap().is_empty());
        assert!(running_client().is_ok());
    }
}
