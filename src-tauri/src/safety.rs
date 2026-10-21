//! Deletion guard. Every filesystem path that Tuntas is about to delete goes through
//! `check_deletable`, which refuses drive roots, OS folders, user profile folders and
//! anything that would take one of those down with it.

use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf, Prefix};

/// Lexically resolves `.` and `..`, strips the `\\?\` verbatim prefix and returns a
/// lowercase, backslash-separated string without a trailing separator.
pub fn normalize(path: &Path) -> Option<String> {
	let mut drive: Option<String> = None;
	let mut parts: Vec<String> = Vec::new();
	for component in path.components() {
		match component {
			Component::Prefix(prefix) => {
				drive = match prefix.kind() {
					Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
						Some(format!("{}:", (letter as char).to_ascii_lowercase()))
					}
					// Network shares and device paths are never deletion targets.
					_ => return None,
				};
			}
			Component::RootDir => {}
			Component::CurDir => {}
			Component::ParentDir => {
				parts.pop();
			}
			Component::Normal(part) => parts.push(part.to_string_lossy().to_lowercase()),
		}
	}
	let drive = drive?;
	if parts.is_empty() {
		return Some(format!("{}\\", drive));
	}
	Some(format!("{}\\{}", drive, parts.join("\\")))
}

fn env_path(name: &str) -> Option<PathBuf> {
	env::var_os(name).filter(|v| !v.is_empty()).map(PathBuf::from)
}

struct Guard {
	/// Paths that may never be deleted themselves (nor any ancestor of them).
	exact: Vec<String>,
	/// Paths whose whole subtree is off limits.
	subtree: Vec<String>,
}

fn build_guard() -> Guard {
	let mut exact: Vec<PathBuf> = Vec::new();
	let mut subtree: Vec<PathBuf> = Vec::new();

	let system_drive = env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
	let system_root = env_path("SystemRoot").unwrap_or_else(|| PathBuf::from(format!("{}\\Windows", system_drive)));
	subtree.push(system_root);
	for name in ["System Volume Information", "$Recycle.Bin", "Recovery", "Boot", "EFI"] {
		subtree.push(PathBuf::from(format!("{}\\{}", system_drive, name)));
	}

	for name in [
		"ProgramFiles",
		"ProgramFiles(x86)",
		"ProgramW6432",
		"CommonProgramFiles",
		"CommonProgramFiles(x86)",
		"CommonProgramW6432",
		"ProgramData",
		"ALLUSERSPROFILE",
		"PUBLIC",
		"USERPROFILE",
		"APPDATA",
		"LOCALAPPDATA",
		"TEMP",
		"TMP",
		"OneDrive",
	] {
		if let Some(p) = env_path(name) {
			exact.push(p);
		}
	}

	if let Some(program_data) = env_path("ProgramData") {
		exact.push(program_data.join("Microsoft"));
		exact.push(program_data.join("Package Cache"));
		subtree.push(program_data.join("Microsoft\\Windows"));
		subtree.push(program_data.join("Microsoft\\Crypto"));
		subtree.push(program_data.join("Microsoft\\Windows Defender"));
	}

	// Every user profile on the machine plus the well-known folders inside the current one.
	if let Some(profile) = env_path("USERPROFILE") {
		if let Some(users_root) = profile.parent() {
			exact.push(users_root.to_path_buf());
			if let Ok(entries) = fs::read_dir(users_root) {
				for entry in entries.flatten() {
					exact.push(entry.path());
				}
			}
		}
		for name in [
			"Desktop",
			"Documents",
			"Downloads",
			"Pictures",
			"Music",
			"Videos",
			"Favorites",
			"Contacts",
			"Links",
			"Saved Games",
			"Searches",
			"OneDrive",
			"AppData",
			"AppData\\LocalLow",
		] {
			exact.push(profile.join(name));
		}
	}
	if let Some(local) = env_path("LOCALAPPDATA") {
		for name in ["Programs", "Programs\\Common", "Packages", "Microsoft", "Temp", "Google", "Mozilla"] {
			exact.push(local.join(name));
		}
		subtree.push(local.join("Microsoft\\Windows"));
		subtree.push(local.join("Microsoft\\Credentials"));
	}
	if let Some(roaming) = env_path("APPDATA") {
		exact.push(roaming.join("Microsoft"));
		for name in ["Windows", "Protect", "Credentials", "Crypto", "SystemCertificates"] {
			subtree.push(roaming.join("Microsoft").join(name));
		}
	}

	// Tuntas never deletes itself.
	if let Ok(exe) = env::current_exe() {
		if let Some(dir) = exe.parent() {
			exact.push(dir.to_path_buf());
		}
	}

	Guard {
		exact: exact.iter().filter_map(|p| normalize(p)).collect(),
		subtree: subtree.iter().filter_map(|p| normalize(p)).collect(),
	}
}

/// Start Menu shortcut folders of programs are fair game even though they live under
/// `Microsoft\Windows`. The Windows-owned groups (Startup, Accessibility, ...) are not.
fn is_start_menu_entry(norm: &str) -> bool {
	const RESERVED: &[&str] = &[
		"startup",
		"accessibility",
		"accessories",
		"administrative tools",
		"maintenance",
		"system tools",
		"windows powershell",
		"windows tools",
		"windows accessories",
		"windows administrative tools",
		"windows system",
		"windows ease of access",
	];
	let bases: Vec<String> = [("APPDATA", "Microsoft\\Windows\\Start Menu\\Programs"), ("ProgramData", "Microsoft\\Windows\\Start Menu\\Programs")]
		.iter()
		.filter_map(|(var, sub)| env_path(var).and_then(|p| normalize(&p.join(sub))))
		.collect();
	bases.iter().any(|base| {
		norm.strip_prefix(&format!("{}\\", base))
			.and_then(|rest| rest.split('\\').next())
			.map(|first| !first.is_empty() && !RESERVED.contains(&first.trim_end_matches(".lnk")))
			.unwrap_or(false)
	})
}

fn is_same_or_inside(path: &str, base: &str) -> bool {
	path == base || path.starts_with(&format!("{}\\", base.trim_end_matches('\\')))
}

/// Returns `Ok(())` when deleting `target` cannot hit anything the OS or the user depends on.
pub fn check_deletable(target: &Path) -> Result<(), String> {
	let display = target.display();
	if !target.is_absolute() {
		return Err(format!("Refused `{}`: not an absolute path.", display));
	}
	let Some(norm) = normalize(target) else {
		return Err(format!("Refused `{}`: network and device paths are not supported.", display));
	};
	// `c:\` is 3 characters long; anything that short is a drive root.
	if norm.len() <= 3 {
		return Err(format!("Refused `{}`: drive roots are protected.", display));
	}

	let guard = build_guard();
	let start_menu_entry = is_start_menu_entry(&norm);
	for protected in &guard.subtree {
		if start_menu_entry && is_same_or_inside(&norm, protected) {
			continue;
		}
		if is_same_or_inside(&norm, protected) || is_same_or_inside(protected, &norm) {
			return Err(format!("Refused `{}`: protected system location.", display));
		}
	}
	for protected in &guard.exact {
		// Deleting an ancestor of a protected folder would delete that folder too.
		if is_same_or_inside(protected, &norm) {
			return Err(format!("Refused `{}`: protected folder or one of its parents.", display));
		}
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn normalizes_parent_segments_and_case() {
		assert_eq!(normalize(Path::new("C:\\Windows\\..\\Users\\X")).as_deref(), Some("c:\\users\\x"));
		assert_eq!(normalize(Path::new("D:\\")).as_deref(), Some("d:\\"));
		assert_eq!(normalize(Path::new("\\\\server\\share\\x")), None);
	}

	#[test]
	fn blocks_roots_system_and_profiles() {
		assert!(check_deletable(Path::new("C:\\")).is_err());
		assert!(check_deletable(Path::new("D:\\")).is_err());
		assert!(check_deletable(Path::new("C:\\Windows\\WinSxS")).is_err());
		assert!(check_deletable(Path::new("C:\\Windows\\..\\Windows\\System32")).is_err());
		if let Some(profile) = env_path("USERPROFILE") {
			assert!(check_deletable(&profile).is_err());
			assert!(check_deletable(&profile.join("Documents")).is_err());
			assert!(check_deletable(profile.parent().unwrap()).is_err());
		}
		if let Some(roaming) = env_path("APPDATA") {
			assert!(check_deletable(&roaming.join("Microsoft")).is_err());
			assert!(check_deletable(&roaming.join("Microsoft\\Windows\\Start Menu")).is_err());
		}
	}

	#[test]
	fn allows_regular_app_folders() {
		assert!(check_deletable(Path::new("C:\\Program Files\\SomeVendor\\SomeApp")).is_ok());
		assert!(check_deletable(Path::new("D:\\Tools\\SomeApp")).is_ok());
		if let Some(roaming) = env_path("APPDATA") {
			assert!(check_deletable(&roaming.join("SomeApp")).is_ok());
			assert!(check_deletable(&roaming.join("Microsoft\\Teams")).is_ok());
			let start_menu = roaming.join("Microsoft\\Windows\\Start Menu\\Programs");
			assert!(check_deletable(&start_menu.join("SomeApp")).is_ok());
			assert!(check_deletable(&start_menu.join("SomeApp.lnk")).is_ok());
			assert!(check_deletable(&start_menu).is_err());
			assert!(check_deletable(&start_menu.join("Startup")).is_err());
		}
	}
}
