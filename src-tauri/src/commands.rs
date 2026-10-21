use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::env;
use std::fs;
use std::os::windows::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread::sleep;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use winreg::enums::*;
use winreg::{RegKey, RegValue};

use crate::safety::{check_deletable, normalize};
use crate::win::{self, hidden_command, LockingProcess};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstalledApp {
	pub id: String,
	pub display_name: String,
	pub display_version: String,
	pub publisher: String,
	pub install_date: String,
	pub install_location: String,
	pub estimated_size_kb: u64,
	pub uninstall_string: String,
	pub quiet_uninstall_string: String,
	pub display_icon: String,
	pub registry_path: String,
	pub architecture: String,
	pub system_component: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LeftoverItem {
	pub id: String,
	pub kind: String,
	pub path: String,
	pub confidence: String,
	pub size_bytes: u64,
	pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedItem {
	pub path: String,
	pub files: Vec<String>,
	pub processes: Vec<LockingProcess>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeletionReport {
	pub removed_count: usize,
	pub freed_bytes: u64,
	pub skipped_in_use: usize,
	pub errors: Vec<String>,
	pub locked: Vec<LockedItem>,
	pub backup_dir: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NativeUninstallResult {
	pub pid: u32,
	pub command: String,
	pub exit_code: Option<i32>,
	pub success: bool,
	pub still_installed: bool,
	pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsStoreApp {
	pub name: String,
	pub package_full_name: String,
	pub version: String,
	pub publisher: String,
	pub install_location: String,
	pub is_framework: bool,
	pub non_removable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartupEntry {
	pub id: String,
	pub name: String,
	pub command: String,
	pub location: String,
	pub source_kind: String,
	pub scope: String,
	pub enabled: bool,
	pub approved_key: String,
	pub approved_value: String,
	pub file_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JunkCategoryItem {
	pub id: String,
	pub category: String,
	pub path: String,
	pub file_count: usize,
	pub total_bytes: u64,
	pub safe_to_clean: bool,
	pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProcessItem {
	pub pid: u32,
	pub name: String,
	pub exe_path: String,
	pub working_set_bytes: u64,
	pub matched_app_name: Option<String>,
	pub matched_registry_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReleaseReport {
	pub released: Vec<u32>,
	pub still_running: Vec<LockingProcess>,
	pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleReport {
	pub scheduled: usize,
	pub errors: Vec<String>,
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Counts files and bytes below `path` without following symlinks or junctions.
fn compute_dir_stats(path: &Path) -> (usize, u64) {
	fn walk(path: &Path, depth: usize, count: &mut usize, bytes: &mut u64) {
		if depth > 64 {
			return;
		}
		let Ok(entries) = fs::read_dir(path) else {
			return;
		};
		for entry in entries.flatten() {
			let Ok(file_type) = entry.file_type() else {
				continue;
			};
			if file_type.is_symlink() {
				continue;
			}
			if file_type.is_dir() {
				walk(&entry.path(), depth + 1, count, bytes);
			} else if let Ok(meta) = entry.metadata() {
				*count += 1;
				*bytes = bytes.saturating_add(meta.len());
			}
		}
	}

	let Ok(meta) = fs::symlink_metadata(path) else {
		return (0, 0);
	};
	if meta.is_file() {
		return (1, meta.len());
	}
	let mut count = 0;
	let mut bytes = 0;
	walk(path, 0, &mut count, &mut bytes);
	(count, bytes)
}

/// Lists up to `limit` files below `path`, used to ask the Restart Manager who locks them.
fn list_files(path: &Path, limit: usize) -> Vec<PathBuf> {
	let mut out = Vec::new();
	let mut stack = vec![path.to_path_buf()];
	if path.is_file() {
		return vec![path.to_path_buf()];
	}
	while let Some(dir) = stack.pop() {
		let Ok(entries) = fs::read_dir(&dir) else {
			continue;
		};
		for entry in entries.flatten() {
			let Ok(file_type) = entry.file_type() else {
				continue;
			};
			if file_type.is_symlink() {
				continue;
			}
			if file_type.is_dir() {
				stack.push(entry.path());
			} else {
				out.push(entry.path());
				if out.len() >= limit {
					return out;
				}
			}
		}
	}
	out
}

fn normalize_token(input: &str) -> String {
	input
		.chars()
		.filter(|c| c.is_alphanumeric())
		.collect::<String>()
		.to_lowercase()
}

fn powershell_exe() -> String {
	if let Ok(sys_root) = env::var("SystemRoot") {
		let p = PathBuf::from(sys_root).join("System32\\WindowsPowerShell\\v1.0\\powershell.exe");
		if p.exists() {
			return p.to_string_lossy().to_string();
		}
	}
	"powershell.exe".to_string()
}

fn run_powershell(script: &str) -> Result<std::process::Output, String> {
	hidden_command(powershell_exe())
		.args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
		.output()
		.map_err(|e| format!("Failed to start PowerShell: {}", e))
}

fn ps_quote(value: &str) -> String {
	format!("'{}'", value.replace('\'', "''"))
}

/// Expands `%VAR%` references the way REG_EXPAND_SZ values expect.
fn expand_env(input: &str) -> String {
	let mut out = String::new();
	let mut rest = input;
	while let Some(start) = rest.find('%') {
		out.push_str(&rest[..start]);
		let after = &rest[start + 1..];
		match after.find('%') {
			Some(end) => {
				let name = &after[..end];
				match env::var(name) {
					Ok(value) if !name.is_empty() => out.push_str(&value),
					_ => {
						out.push('%');
						out.push_str(name);
						out.push('%');
					}
				}
				rest = &after[end + 1..];
			}
			None => {
				out.push('%');
				rest = after;
			}
		}
	}
	out.push_str(rest);
	out
}

fn unix_now() -> Duration {
	SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default()
}

/// `YYYYMMDD-HHMMSS` in UTC, without pulling in a date crate.
fn timestamp_label() -> String {
	let secs = unix_now().as_secs() as i64;
	let days = secs.div_euclid(86_400);
	let rem = secs.rem_euclid(86_400);
	// Howard Hinnant's civil_from_days.
	let z = days + 719_468;
	let era = z.div_euclid(146_097);
	let doe = z - era * 146_097;
	let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
	let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
	let mp = (5 * doy + 2) / 153;
	let day = doy - (153 * mp + 2) / 5 + 1;
	let month = if mp < 10 { mp + 3 } else { mp - 9 };
	let year = yoe + era * 400 + if month <= 2 { 1 } else { 0 };
	format!(
		"{:04}{:02}{:02}-{:02}{:02}{:02}",
		year,
		month,
		day,
		rem / 3_600,
		(rem % 3_600) / 60,
		rem % 60
	)
}

fn backups_root() -> PathBuf {
	let base = env::var("LOCALAPPDATA").map(PathBuf::from).unwrap_or_else(|_| env::temp_dir());
	base.join("Tuntas").join("Backups")
}

/// Splits `HKLM\SOFTWARE\...` into a predefined hive and its subkey path.
fn split_registry_path(path: &str) -> Option<(RegKey, String)> {
	let (hive, sub) = path.split_once('\\')?;
	let hive = match hive.to_ascii_uppercase().as_str() {
		"HKLM" | "HKEY_LOCAL_MACHINE" => HKEY_LOCAL_MACHINE,
		"HKCU" | "HKEY_CURRENT_USER" => HKEY_CURRENT_USER,
		_ => return None,
	};
	Some((RegKey::predef(hive), sub.trim_matches('\\').to_string()))
}

fn registry_key_exists(path: &str) -> bool {
	split_registry_path(path)
		.map(|(root, sub)| root.open_subkey_with_flags(sub, KEY_READ).is_ok())
		.unwrap_or(false)
}

fn elevation_hint() -> &'static str {
	if win::is_elevated() {
		""
	} else {
		" Restart Tuntas as administrator to remove machine-wide items."
	}
}

// ---------------------------------------------------------------------------
// Elevation, restore points, backups
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn is_elevated() -> bool {
	win::is_elevated()
}

#[tauri::command(async)]
pub fn restart_as_admin(app: tauri::AppHandle) -> Result<(), String> {
	win::relaunch_elevated()?;
	app.exit(0);
	Ok(())
}

#[tauri::command(async)]
pub fn create_restore_point(description: String) -> Result<String, String> {
	if !win::is_elevated() {
		return Err("Creating a restore point needs administrator rights.".to_string());
	}
	let desc: String = description.chars().take(200).collect();
	let script = format!(
		"Checkpoint-Computer -Description {} -RestorePointType APPLICATION_UNINSTALL -WarningVariable w -WarningAction SilentlyContinue -ErrorAction Stop; if ($w) {{ Write-Output ('WARN:' + $w[0]) }}",
		ps_quote(&desc)
	);
	let out = run_powershell(&script)?;
	if !out.status.success() {
		let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
		return Err(format!(
			"Windows did not create a restore point. System Protection may be turned off for this drive. {}",
			stderr
		));
	}
	let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
	if let Some(warning) = stdout.strip_prefix("WARN:") {
		// Windows allows one restore point per 24 hours by default and reports the skip as a warning.
		return Ok(format!("No new restore point: {}", warning.trim()));
	}
	Ok("Restore point created.".to_string())
}

#[tauri::command(async)]
pub fn open_backup_folder() -> Result<String, String> {
	let root = backups_root();
	fs::create_dir_all(&root).map_err(|e| format!("Cannot create backup folder: {}", e))?;
	open_in_explorer(root.display().to_string())
}

// ---------------------------------------------------------------------------
// Installed programs and native uninstall
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn list_installed_apps() -> Result<Vec<InstalledApp>, String> {
	let mut apps: Vec<InstalledApp> = Vec::new();
	let roots = [
		(HKEY_LOCAL_MACHINE, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall", "HKLM", "x64"),
		(
			HKEY_LOCAL_MACHINE,
			"SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall",
			"HKLM",
			"x86",
		),
		(HKEY_CURRENT_USER, "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall", "HKCU", "User"),
	];

	for (hive, subkey_path, hive_label, arch_label) in roots {
		let root = RegKey::predef(hive);
		let Ok(uninst_key) = root.open_subkey_with_flags(subkey_path, KEY_READ) else {
			continue;
		};
		for sub_name in uninst_key.enum_keys().flatten() {
			let Ok(app_key) = uninst_key.open_subkey_with_flags(&sub_name, KEY_READ) else {
				continue;
			};
			let display_name: String = app_key.get_value("DisplayName").unwrap_or_default();
			if display_name.trim().is_empty() {
				continue;
			}
			// Updates and hotfixes belong to their parent product, not to the program list.
			let parent_key: String = app_key.get_value("ParentKeyName").unwrap_or_default();
			let release_type: String = app_key.get_value("ReleaseType").unwrap_or_default();
			if !parent_key.is_empty() || matches!(release_type.as_str(), "Update" | "Hotfix" | "Security Update") {
				continue;
			}
			let system_component: u32 = app_key.get_value("SystemComponent").unwrap_or(0);
			let estimated_size_kb: u32 = app_key.get_value("EstimatedSize").unwrap_or(0);
			let get = |name: &str| -> String {
				let value: String = app_key.get_value(name).unwrap_or_default();
				value.trim().to_string()
			};

			apps.push(InstalledApp {
				id: format!("{}:{}:{}", hive_label, arch_label, sub_name),
				display_name: display_name.trim().to_string(),
				display_version: get("DisplayVersion"),
				publisher: get("Publisher"),
				install_date: get("InstallDate"),
				install_location: get("InstallLocation").trim_matches('"').to_string(),
				estimated_size_kb: u64::from(estimated_size_kb),
				uninstall_string: get("UninstallString"),
				quiet_uninstall_string: get("QuietUninstallString"),
				display_icon: get("DisplayIcon"),
				registry_path: format!("{}\\{}\\{}", hive_label, subkey_path, sub_name),
				architecture: arch_label.to_string(),
				system_component: system_component == 1,
			});
		}
	}

	apps.sort_by(|a, b| a.display_name.to_lowercase().cmp(&b.display_name.to_lowercase()));
	apps.dedup_by(|a, b| {
		a.display_name.eq_ignore_ascii_case(&b.display_name)
			&& a.display_version == b.display_version
			&& a.architecture == b.architecture
	});
	Ok(apps)
}

/// Splits a registry command line into program and arguments the way CreateProcess does:
/// a quoted program, or the shortest space-separated prefix that names an existing file.
fn split_command_line(cmd: &str) -> (String, String) {
	let cmd = cmd.trim();
	if let Some(rest) = cmd.strip_prefix('"') {
		return match rest.find('"') {
			Some(end) => (rest[..end].to_string(), rest[end + 1..].trim().to_string()),
			None => (rest.to_string(), String::new()),
		};
	}
	let names_file = |candidate: &str| {
		let p = Path::new(candidate);
		p.is_file() || (p.extension().is_none() && Path::new(&format!("{}.exe", candidate)).is_file())
	};
	for (i, c) in cmd.char_indices() {
		if c == ' ' && names_file(&cmd[..i]) {
			return (cmd[..i].to_string(), cmd[i + 1..].trim().to_string());
		}
	}
	if names_file(cmd) {
		return (cmd.to_string(), String::new());
	}
	match cmd.split_once(' ') {
		Some((exe, args)) => (exe.to_string(), args.trim().to_string()),
		None => (cmd.to_string(), String::new()),
	}
}

fn is_msiexec(exe: &str) -> bool {
	let name = Path::new(exe)
		.file_name()
		.map(|n| n.to_string_lossy().to_lowercase())
		.unwrap_or_default();
	name == "msiexec" || name == "msiexec.exe"
}

/// Many MSI products register `MsiExec.exe /I{GUID}`, which opens the repair/modify
/// wizard. Uninstalling needs `/X{GUID}`.
fn fix_msi_arguments(args: &str, add_quiet: bool) -> String {
	let mut fixed = String::with_capacity(args.len() + 16);
	let chars: Vec<char> = args.chars().collect();
	let mut i = 0;
	while i < chars.len() {
		if (chars[i] == '/' || chars[i] == '-')
			&& i + 1 < chars.len()
			&& chars[i + 1].eq_ignore_ascii_case(&'i')
			&& matches!(chars.get(i + 2), Some('{') | Some(' '))
		{
			fixed.push('/');
			fixed.push('X');
			i += 2;
			continue;
		}
		fixed.push(chars[i]);
		i += 1;
	}
	let lower = fixed.to_lowercase();
	if add_quiet && !lower.contains("/q") {
		fixed.push_str(" /qb /norestart");
	}
	fixed
}

/// Helper processes that installers spawn from %TEMP% and then hand control to.
fn looks_like_uninstall_helper(name: &str, exe_path: Option<&str>, temp_dir: &str) -> bool {
	let lower = name.to_lowercase();
	let named_like_helper = lower.starts_with("_iu")
		|| lower.starts_with("au_")
		|| lower.starts_with("un_")
		|| lower.starts_with("unins")
		|| lower.ends_with(".tmp");
	let from_temp = exe_path
		.map(|p| p.to_lowercase().starts_with(temp_dir))
		.unwrap_or(false);
	named_like_helper && from_temp
}

/// Programs an uninstaller may open at the end (survey pages, Explorer) that must not keep us waiting.
fn is_detached_companion(name: &str) -> bool {
	matches!(
		name.to_lowercase().as_str(),
		"chrome.exe"
			| "msedge.exe"
			| "firefox.exe"
			| "brave.exe"
			| "opera.exe"
			| "iexplore.exe"
			| "explorer.exe"
			| "conhost.exe"
			| "msedgewebview2.exe"
	)
}

#[tauri::command(async)]
pub fn run_native_uninstaller(
	uninstall_string: String,
	quiet_uninstall_string: String,
	prefer_quiet: bool,
	registry_path: Option<String>,
) -> Result<NativeUninstallResult, String> {
	let use_quiet_string = prefer_quiet && !quiet_uninstall_string.trim().is_empty();
	let raw = if use_quiet_string {
		quiet_uninstall_string.trim()
	} else {
		uninstall_string.trim()
	};
	if raw.is_empty() {
		return Err("This program has no UninstallString. Use Forced Uninstall to remove it.".to_string());
	}

	let (exe, mut args) = split_command_line(&expand_env(raw));
	if is_msiexec(&exe) {
		args = fix_msi_arguments(&args, prefer_quiet && !use_quiet_string);
	}
	let display_command = if args.is_empty() {
		format!("\"{}\"", exe)
	} else {
		format!("\"{}\" {}", exe, args)
	};

	let baseline: HashSet<u32> = win::snapshot_processes().iter().map(|p| p.pid).collect();
	let temp_dir = env::temp_dir()
		.to_string_lossy()
		.to_lowercase()
		.trim_end_matches('\\')
		.to_string();

	let mut command = Command::new(&exe);
	if !args.is_empty() {
		command.raw_arg(&args);
	}
	let mut child = command
		.spawn()
		.map_err(|e| format!("Failed to start the uninstaller `{}`: {}", display_command, e))?;
	let root_pid = child.id();

	// Inno Setup, NSIS and others copy themselves to %TEMP%, start the copy and exit at once.
	// Track every descendant (and such helpers) and wait until all of them are gone.
	let mut tracked: HashSet<u32> = HashSet::from([root_pid]);
	let mut exit_code: Option<i32> = None;
	let mut root_done = false;
	let started = Instant::now();
	let max_wait = Duration::from_secs(3 * 60 * 60);
	let mut timed_out = false;

	loop {
		if !root_done {
			if let Ok(Some(status)) = child.try_wait() {
				exit_code = status.code();
				root_done = true;
			}
		}

		let processes = win::snapshot_processes();
		for p in &processes {
			if baseline.contains(&p.pid) || tracked.contains(&p.pid) || is_detached_companion(&p.name) {
				continue;
			}
			if tracked.contains(&p.parent_pid) {
				tracked.insert(p.pid);
				continue;
			}
			let (path, _) = win::process_details(p.pid);
			if looks_like_uninstall_helper(&p.name, path.as_deref(), &temp_dir) {
				tracked.insert(p.pid);
			}
		}
		let helpers_alive = processes
			.iter()
			.any(|p| p.pid != root_pid && tracked.contains(&p.pid) && !is_detached_companion(&p.name));

		if root_done && !helpers_alive {
			break;
		}
		if started.elapsed() > max_wait {
			timed_out = true;
			break;
		}
		let interval = if started.elapsed() < Duration::from_secs(10) { 150 } else { 500 };
		sleep(Duration::from_millis(interval));
	}

	// Give the uninstaller a moment to finish deleting its registry entry.
	sleep(Duration::from_millis(800));
	let still_installed = registry_path
		.as_deref()
		.map(registry_key_exists)
		.unwrap_or(false);
	// 3010 and 1641 are MSI's "success, restart required" codes; 1605 means "already removed".
	let exit_ok = matches!(exit_code, Some(0) | Some(3010) | Some(1641) | Some(1605));
	let success = !timed_out && !still_installed && (exit_ok || registry_path.is_some());

	let message = if timed_out {
		"The uninstaller is still running after 3 hours. Tuntas stopped waiting for it.".to_string()
	} else if still_installed {
		"The uninstaller closed, but the program is still registered. It was probably cancelled or failed.".to_string()
	} else {
		format!(
			"Uninstaller finished with exit code {}.",
			exit_code.map(|c| c.to_string()).unwrap_or_else(|| "unknown".to_string())
		)
	};

	Ok(NativeUninstallResult {
		pid: root_pid,
		command: display_command,
		exit_code,
		success,
		still_installed,
		message,
	})
}

// ---------------------------------------------------------------------------
// Leftover scanning
// ---------------------------------------------------------------------------

/// Folder and key names that hold data for many programs. They are never matched directly.
const GENERIC_NAMES: &[&str] = &[
	"microsoft", "windows", "google", "mozilla", "adobe", "apple", "intel", "amd", "nvidia", "nvidiacorporation",
	"programs", "packages", "packagecache", "temp", "tmp", "cache", "caches", "data", "common", "commonfiles",
	"installer", "installers", "apps", "app", "application", "applications", "users", "user", "shared", "tools",
	"bin", "lib", "logs", "log", "config", "settings", "update", "updater", "updates", "downloads", "documents",
	"desktop", "crashdumps", "crashpad", "squirreltemp", "virtualstore", "publishers", "history", "inetcache",
	"d3dscache", "fontcache", "connecteddevicesplatform", "comms", "windowsapps", "systemcertificates",
	"credentials", "vault", "assembly", "dotnet", "classes", "clients", "policies", "wow6432node",
	"registeredapplications", "odbc", "partner", "startmenu", "programdata", "local", "roaming", "locallow",
];

const ARCH_WORDS: &[&str] = &[
	"x64", "x86", "x8664", "amd64", "arm64", "win64", "win32", "64bit", "32bit", "64", "32",
];

const PUBLISHER_SUFFIXES: &[&str] = &[
	"corporation", "corp", "inc", "incorporated", "llc", "ltd", "limited", "gmbh", "co", "company", "foundation",
	"ag", "sa", "bv", "srl", "pte", "pty", "plc", "oy", "ab", "as", "kg", "sro", "spa", "the",
];

fn is_generic(token: &str) -> bool {
	token.len() < 3 || GENERIC_NAMES.contains(&token)
}

fn is_version_like(word: &str) -> bool {
	let w = word.strip_prefix('v').unwrap_or(word);
	!w.is_empty() && w.chars().any(|c| c.is_ascii_digit()) && w.chars().all(|c| c.is_ascii_digit() || c == '.' || c == '-')
}

/// Drops bracketed parts, version numbers and architecture tags:
/// `7-Zip 23.01 (x64)` becomes `7-Zip`.
fn clean_display_name(name: &str) -> String {
	let mut without_brackets = String::new();
	let mut depth = 0usize;
	for c in name.chars() {
		match c {
			'(' | '[' => depth += 1,
			')' | ']' => depth = depth.saturating_sub(1),
			_ if depth == 0 => without_brackets.push(c),
			_ => {}
		}
	}
	without_brackets
		.split_whitespace()
		.filter(|word| {
			let lw = word.to_lowercase();
			let token = normalize_token(&lw);
			!(token.is_empty() || is_version_like(&lw) || ARCH_WORDS.contains(&token.as_str()) || token == "version")
		})
		.collect::<Vec<_>>()
		.join(" ")
}

fn clean_publisher(publisher: &str) -> String {
	publisher
		.split(|c: char| c.is_whitespace() || c == ',' || c == '.')
		.map(normalize_token)
		.filter(|w| !w.is_empty() && !PUBLISHER_SUFFIXES.contains(&w.as_str()))
		.collect::<String>()
}

struct AppTokens {
	/// Cleaned display name, e.g. `mozillafirefox`.
	name: String,
	/// Name without the publisher prefix, e.g. `firefox`.
	core: String,
	/// Cleaned publisher, e.g. `mozilla`.
	publisher: String,
}

impl AppTokens {
	fn new(display_name: &str, publisher: &str) -> Self {
		let name = normalize_token(&clean_display_name(display_name));
		let publisher = clean_publisher(publisher);
		let core = if publisher.len() >= 3 && name.len() > publisher.len() && name.starts_with(&publisher) {
			name[publisher.len()..].to_string()
		} else {
			String::new()
		};
		AppTokens { name, core, publisher }
	}

	/// A folder or key directly under a shared root (AppData, SOFTWARE, ...) belongs to the
	/// app only when its name is the app name, not merely part of it.
	fn matches_top_level(&self, candidate: &str) -> bool {
		if is_generic(candidate) {
			return false;
		}
		(self.name.len() >= 3 && candidate == self.name) || (self.core.len() >= 5 && candidate == self.core)
	}

	/// Inside the publisher's own folder the core name alone is enough (`Mozilla\Firefox`).
	fn matches_under_publisher(&self, candidate: &str) -> bool {
		if is_generic(candidate) {
			return false;
		}
		(self.name.len() >= 3 && candidate == self.name) || (self.core.len() >= 3 && candidate == self.core)
	}

	fn is_publisher(&self, candidate: &str) -> bool {
		self.publisher.len() >= 3 && candidate == self.publisher
	}
}

fn push_folder(items: &mut Vec<LeftoverItem>, path: &Path, confidence: &str, reason: String) {
	if check_deletable(path).is_err() {
		return;
	}
	let (_, bytes) = compute_dir_stats(path);
	items.push(LeftoverItem {
		id: format!("fs:{}", path.display()),
		kind: if path.is_dir() { "folder" } else { "file" }.to_string(),
		path: path.display().to_string(),
		confidence: confidence.to_string(),
		size_bytes: bytes,
		reason,
	});
}

fn scan_leftovers(
	target_name: &str,
	publisher: &str,
	target_path: &str,
	registry_path: Option<&str>,
	uninstall_string: Option<&str>,
) -> Vec<LeftoverItem> {
	let mut items: Vec<LeftoverItem> = Vec::new();
	let tokens = AppTokens::new(target_name, publisher);

	// 1. The registered install folder.
	let install_dir = target_path.trim().trim_matches('"');
	if !install_dir.is_empty() {
		let p = PathBuf::from(install_dir);
		if p.exists() {
			let folder = normalize_token(&p.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default());
			let uninstaller_inside = uninstall_string
				.map(|u| split_command_line(&expand_env(u)).0)
				.and_then(|exe| normalize(Path::new(&exe)))
				.zip(normalize(&p))
				.map(|(exe, dir)| exe.starts_with(&format!("{}\\", dir)))
				.unwrap_or(false);
			let name_fits = !is_generic(&folder)
				&& (folder == tokens.name
					|| (tokens.core.len() >= 3 && folder == tokens.core)
					|| (tokens.name.len() >= 4 && folder.contains(&tokens.name))
					|| (tokens.core.len() >= 4 && folder.contains(&tokens.core)));
			if uninstaller_inside || name_fits {
				push_folder(&mut items, &p, "exact-path", "Registered install folder still exists".to_string());
			} else {
				push_folder(
					&mut items,
					&p,
					"registered-path",
					"Registered install folder, but its name does not match the program. Check it before deleting."
						.to_string(),
				);
			}
		}
	}

	// 2. Folders named after the app (or Publisher\App) in the usual data locations.
	let mut roots: Vec<(String, PathBuf)> = Vec::new();
	for (label, var, sub) in [
		("AppData\\Local", "LOCALAPPDATA", ""),
		("AppData\\Local\\Programs", "LOCALAPPDATA", "Programs"),
		("AppData\\Roaming", "APPDATA", ""),
		("AppData\\LocalLow", "USERPROFILE", "AppData\\LocalLow"),
		("ProgramData", "ProgramData", ""),
		("Program Files", "ProgramFiles", ""),
		("Program Files (x86)", "ProgramFiles(x86)", ""),
		("Start Menu", "APPDATA", "Microsoft\\Windows\\Start Menu\\Programs"),
		("Start Menu (all users)", "ProgramData", "Microsoft\\Windows\\Start Menu\\Programs"),
	] {
		if let Ok(base) = env::var(var) {
			let root = if sub.is_empty() { PathBuf::from(base) } else { PathBuf::from(base).join(sub) };
			roots.push((label.to_string(), root));
		}
	}

	for (label, root) in &roots {
		let Ok(entries) = fs::read_dir(root) else {
			continue;
		};
		for entry in entries.flatten() {
			let path = entry.path();
			let file_name = entry.file_name().to_string_lossy().to_string();
			let is_dir = path.is_dir();
			let stem = if is_dir {
				file_name.clone()
			} else if file_name.to_lowercase().ends_with(".lnk") {
				file_name[..file_name.len() - 4].to_string()
			} else {
				continue;
			};
			let token = normalize_token(&clean_display_name(&stem));

			if tokens.matches_top_level(&token) {
				push_folder(&mut items, &path, "name-match", format!("{} entry named after the program", label));
			} else if is_dir && tokens.is_publisher(&token) {
				let Ok(sub_entries) = fs::read_dir(&path) else {
					continue;
				};
				for sub in sub_entries.flatten() {
					let sub_name = sub.file_name().to_string_lossy().to_string();
					let sub_stem = sub_name.strip_suffix(".lnk").unwrap_or(&sub_name);
					if tokens.matches_under_publisher(&normalize_token(&clean_display_name(sub_stem))) {
						push_folder(
							&mut items,
							&sub.path(),
							"publisher-match",
							format!("Inside the publisher folder `{}` in {}", file_name, label),
						);
					}
				}
			}
		}
	}

	// 3. The program's own uninstall key, if the uninstaller left it behind.
	if let Some(reg) = registry_path.map(str::trim).filter(|r| !r.is_empty()) {
		if registry_key_exists(reg) {
			items.push(LeftoverItem {
				id: format!("reg:{}", reg),
				kind: "registry".to_string(),
				path: reg.to_string(),
				confidence: "exact-path".to_string(),
				size_bytes: 0,
				reason: "Uninstall entry left in the registry".to_string(),
			});
		}
	}

	// 4. Software keys named after the app or Publisher\App.
	let reg_roots = [
		(HKEY_CURRENT_USER, "SOFTWARE", "HKCU\\SOFTWARE"),
		(HKEY_LOCAL_MACHINE, "SOFTWARE", "HKLM\\SOFTWARE"),
		(HKEY_LOCAL_MACHINE, "SOFTWARE\\WOW6432Node", "HKLM\\SOFTWARE\\WOW6432Node"),
	];
	for (hive, sub, prefix) in reg_roots {
		let Ok(soft_key) = RegKey::predef(hive).open_subkey_with_flags(sub, KEY_READ) else {
			continue;
		};
		for key_name in soft_key.enum_keys().flatten() {
			let key_token = normalize_token(&key_name);
			if tokens.matches_top_level(&key_token) {
				let full = format!("{}\\{}", prefix, key_name);
				items.push(LeftoverItem {
					id: format!("reg:{}", full),
					kind: "registry".to_string(),
					path: full,
					confidence: "name-match".to_string(),
					size_bytes: 0,
					reason: "Registry key named after the program".to_string(),
				});
			} else if tokens.is_publisher(&key_token) {
				let Ok(pub_key) = soft_key.open_subkey_with_flags(&key_name, KEY_READ) else {
					continue;
				};
				for child in pub_key.enum_keys().flatten() {
					if tokens.matches_under_publisher(&normalize_token(&child)) {
						let full = format!("{}\\{}\\{}", prefix, key_name, child);
						items.push(LeftoverItem {
							id: format!("reg:{}", full),
							kind: "registry".to_string(),
							path: full,
							confidence: "publisher-match".to_string(),
							size_bytes: 0,
							reason: format!("Inside the publisher key `{}`", key_name),
						});
					}
				}
			}
		}
	}

	let mut seen = HashSet::new();
	items.retain(|item| seen.insert(item.id.to_lowercase()));
	items
}

#[tauri::command(async)]
pub fn scan_app_leftovers(
	display_name: String,
	publisher: String,
	install_location: String,
	registry_path: String,
	uninstall_string: Option<String>,
) -> Result<Vec<LeftoverItem>, String> {
	Ok(scan_leftovers(
		&display_name,
		&publisher,
		&install_location,
		Some(&registry_path),
		uninstall_string.as_deref(),
	))
}

#[tauri::command(async)]
pub fn forced_uninstall_scan(
	target_name: String,
	publisher: String,
	target_path: String,
	registry_path: Option<String>,
) -> Result<Vec<LeftoverItem>, String> {
	let path = target_path.trim();
	if !path.is_empty() {
		check_deletable(Path::new(path.trim_matches('"')))?;
	}
	Ok(scan_leftovers(&target_name, &publisher, path, registry_path.as_deref(), None))
}

// ---------------------------------------------------------------------------
// Deleting leftovers
// ---------------------------------------------------------------------------

/// Only keys below a program-specific level may be deleted, e.g. `HKLM\SOFTWARE\Vendor`
/// but never `HKLM\SOFTWARE` or `HKLM\SOFTWARE\Microsoft`.
fn check_registry_deletable(path: &str) -> Result<(), String> {
	let Some((_, sub)) = split_registry_path(path) else {
		return Err(format!("Refused `{}`: only HKCU and HKLM keys can be removed.", path));
	};
	let parts: Vec<String> = sub.split('\\').map(|p| p.to_lowercase()).collect();
	if parts.first().map(String::as_str) != Some("software") {
		return Err(format!("Refused `{}`: only keys under SOFTWARE can be removed.", path));
	}
	let mut rest = &parts[1..];
	if rest.first().map(String::as_str) == Some("wow6432node") {
		rest = &rest[1..];
	}
	let Some(first) = rest.first() else {
		return Err(format!("Refused `{}`: this is a root key.", path));
	};
	let uninstall_prefix = ["microsoft", "windows", "currentversion", "uninstall"];
	let is_uninstall_entry = rest.len() == 5 && rest[..4].iter().zip(uninstall_prefix).all(|(a, b)| a == b);
	if is_uninstall_entry {
		return Ok(());
	}
	if !is_generic(&normalize_token(first)) {
		return Ok(());
	}
	// Below a shared vendor key (`SOFTWARE\Microsoft\Teams`) one more level is required,
	// and the OS's own subkeys stay off limits.
	match rest.get(1).map(|s| normalize_token(s)) {
		Some(second) if !is_generic(&second) && !PROTECTED_VENDOR_SUBKEYS.contains(&second.as_str()) => Ok(()),
		_ => Err(format!("Refused `{}`: shared key used by Windows or many programs.", path)),
	}
}

const PROTECTED_VENDOR_SUBKEYS: &[&str] = &[
	"windowsnt", "windowsdefender", "windowsupdate", "windowssearch", "cryptography", "ole", "rpc", "com3",
	"securitycenter", "wbem", "netframework", "activesetup", "internetexplorer", "edge", "edgeupdate", "office",
	"onedrive", "input", "ctf", "speech", "ime", "fusion", "msi", "appmodel", "provisioning", "policymanager",
	"sqmclient", "tracing", "wcmsvc", "dfs", "devdiv", "visualstudio", "vscommon", "directx", "dotnet",
];

fn delete_registry_item(path: &str, backup_dir: &mut Option<PathBuf>, index: usize) -> Result<(), String> {
	check_registry_deletable(path)?;
	if !registry_key_exists(path) {
		return Ok(());
	}
	let dir = match backup_dir {
		Some(dir) => dir.clone(),
		None => {
			let dir = backups_root().join(timestamp_label());
			fs::create_dir_all(&dir).map_err(|e| format!("Cannot create registry backup folder: {}", e))?;
			*backup_dir = Some(dir.clone());
			dir
		}
	};
	let file = dir.join(format!("{:03}-{}.reg", index, normalize_token(path.rsplit('\\').next().unwrap_or("key"))));
	let export = hidden_command("reg")
		.arg("export")
		.arg(path)
		.arg(&file)
		.arg("/y")
		.output()
		.map_err(|e| format!("Failed to run reg.exe: {}", e))?;
	if !export.status.success() {
		return Err(format!(
			"Backup of `{}` failed, so it was not deleted: {}",
			path,
			String::from_utf8_lossy(&export.stderr).trim()
		));
	}
	let delete = hidden_command("reg")
		.args(["delete", path, "/f"])
		.output()
		.map_err(|e| format!("Failed to run reg.exe: {}", e))?;
	if !delete.status.success() {
		return Err(format!(
			"Could not delete `{}`: {}{}",
			path,
			String::from_utf8_lossy(&delete.stderr).trim(),
			elevation_hint()
		));
	}
	Ok(())
}

#[tauri::command(async)]
pub fn delete_leftover_items(items: Vec<LeftoverItem>, permanent: Option<bool>) -> Result<DeletionReport, String> {
	let permanent = permanent.unwrap_or(false);
	let mut report = DeletionReport::default();
	let mut backup_dir: Option<PathBuf> = None;

	for (index, item) in items.iter().enumerate() {
		if item.kind == "registry" {
			match delete_registry_item(&item.path, &mut backup_dir, index) {
				Ok(()) => report.removed_count += 1,
				Err(e) => report.errors.push(e),
			}
			continue;
		}

		let p = PathBuf::from(&item.path);
		if let Err(e) = check_deletable(&p) {
			report.errors.push(e);
			continue;
		}
		if !p.exists() {
			continue;
		}
		let (_, bytes) = compute_dir_stats(&p);
		let result = if permanent {
			(if p.is_dir() { fs::remove_dir_all(&p) } else { fs::remove_file(&p) }).map_err(|e| e.to_string())
		} else {
			win::recycle(&p)
		};
		match result {
			Ok(()) => {
				report.removed_count += 1;
				report.freed_bytes = report.freed_bytes.saturating_add(bytes);
			}
			Err(e) => {
				let remaining = list_files(&p, 4000);
				let processes = win::find_lockers(&remaining);
				if processes.is_empty() {
					report.errors.push(format!("Could not delete `{}`: {}.{}", item.path, e, elevation_hint()));
				} else {
					report.locked.push(LockedItem {
						path: item.path.clone(),
						files: remaining.iter().take(20).map(|f| f.display().to_string()).collect(),
						processes,
					});
				}
			}
		}
	}

	report.backup_dir = backup_dir.map(|d| d.display().to_string());
	Ok(report)
}

/// Processes that must never be closed, whatever the Restart Manager says.
fn is_critical_process(name: &str, pid: u32) -> bool {
	pid <= 4
		|| pid == std::process::id()
		|| matches!(
			name.to_lowercase().as_str(),
			"system"
				| "registry"
				| "smss.exe"
				| "csrss.exe"
				| "wininit.exe"
				| "winlogon.exe"
				| "services.exe"
				| "lsass.exe"
				| "lsaiso.exe"
				| "svchost.exe"
				| "dwm.exe"
				| "fontdrvhost.exe"
				| "memory compression"
		)
}

fn wait_for_exit(pid: u32, timeout: Duration) -> bool {
	let started = Instant::now();
	while started.elapsed() < timeout {
		if !win::is_running(pid) {
			return true;
		}
		sleep(Duration::from_millis(250));
	}
	!win::is_running(pid)
}

#[tauri::command(async)]
pub fn release_locks(processes: Vec<LockingProcess>, force: bool) -> Result<ReleaseReport, String> {
	let mut report = ReleaseReport {
		released: Vec::new(),
		still_running: Vec::new(),
		errors: Vec::new(),
	};

	for proc in processes {
		if !proc.service_name.is_empty() {
			let out = hidden_command("sc").args(["stop", &proc.service_name]).output();
			match out {
				Ok(_) if wait_for_exit(proc.pid, Duration::from_secs(15)) => report.released.push(proc.pid),
				Ok(_) => {
					report.errors.push(format!("Service `{}` did not stop.{}", proc.service_name, elevation_hint()));
					report.still_running.push(proc);
				}
				Err(e) => report.errors.push(format!("Failed to run sc.exe: {}", e)),
			}
			continue;
		}

		if is_critical_process(&proc.name, proc.pid) {
			report.errors.push(format!("`{}` is a Windows system process and cannot be closed.", proc.name));
			report.still_running.push(proc);
			continue;
		}

		if proc.name.eq_ignore_ascii_case("explorer.exe") {
			// Shell extensions live inside Explorer; restarting it releases their DLLs.
			let _ = hidden_command("taskkill").args(["/F", "/PID", &proc.pid.to_string()]).output();
			wait_for_exit(proc.pid, Duration::from_secs(5));
			let explorer = env::var("SystemRoot")
				.map(|r| PathBuf::from(r).join("explorer.exe"))
				.unwrap_or_else(|_| PathBuf::from("explorer.exe"));
			let _ = Command::new(explorer).spawn();
			report.released.push(proc.pid);
			continue;
		}

		if !force && win::request_close(proc.pid) > 0 && wait_for_exit(proc.pid, Duration::from_secs(8)) {
			report.released.push(proc.pid);
			continue;
		}
		if force {
			let _ = hidden_command("taskkill").args(["/F", "/PID", &proc.pid.to_string()]).output();
			if wait_for_exit(proc.pid, Duration::from_secs(5)) {
				report.released.push(proc.pid);
				continue;
			}
		}
		report.still_running.push(proc);
	}

	Ok(report)
}

#[tauri::command(async)]
pub fn schedule_delete_on_reboot(paths: Vec<String>) -> Result<ScheduleReport, String> {
	if !win::is_elevated() {
		return Err("Deleting on restart needs administrator rights.".to_string());
	}
	let mut report = ScheduleReport { scheduled: 0, errors: Vec::new() };
	for raw in paths {
		let root = PathBuf::from(&raw);
		if let Err(e) = check_deletable(&root) {
			report.errors.push(e);
			continue;
		}
		if !root.exists() {
			continue;
		}
		// Files first, then folders from the deepest up, so each folder is empty when its turn comes.
		let files = list_files(&root, usize::MAX);
		let mut dirs: Vec<PathBuf> = Vec::new();
		if root.is_dir() {
			let mut stack = vec![root.clone()];
			while let Some(dir) = stack.pop() {
				if let Ok(entries) = fs::read_dir(&dir) {
					for entry in entries.flatten() {
						if entry.file_type().map(|t| t.is_dir() && !t.is_symlink()).unwrap_or(false) {
							stack.push(entry.path());
						}
					}
				}
				dirs.push(dir);
			}
			dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
		}
		let mut failed = false;
		for target in files.iter().chain(dirs.iter()) {
			if let Err(e) = win::delete_on_reboot(target) {
				report.errors.push(e);
				failed = true;
				break;
			}
		}
		if !failed {
			report.scheduled += 1;
		}
	}
	Ok(report)
}

// ---------------------------------------------------------------------------
// Windows Store apps
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn list_windows_store_apps() -> Result<Vec<WindowsStoreApp>, String> {
	let mut result: Vec<WindowsStoreApp> = Vec::new();
	let script = "[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Get-AppxPackage | Where-Object { -not $_.IsFramework } | Select-Object Name, PackageFullName, Version, Publisher, InstallLocation, @{n='NonRemovable';e={[bool]$_.NonRemovable}}, @{n='SignatureKind';e={[string]$_.SignatureKind}} | ConvertTo-Json -Compress";

	if let Ok(out) = run_powershell(script) {
		let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
		if out.status.success() && !stdout.is_empty() {
			if let Ok(raw) = serde_json::from_str::<serde_json::Value>(&stdout) {
				let arr = match raw {
					serde_json::Value::Array(a) => a,
					obj @ serde_json::Value::Object(_) => vec![obj],
					_ => Vec::new(),
				};
				for item in arr {
					let text = |key: &str| item.get(key).and_then(|v| v.as_str()).unwrap_or_default().to_string();
					let name = text("Name");
					if name.is_empty() {
						continue;
					}
					let non_removable = item.get("NonRemovable").and_then(|v| v.as_bool()).unwrap_or(false)
						|| text("SignatureKind").eq_ignore_ascii_case("System");
					result.push(WindowsStoreApp {
						name,
						package_full_name: text("PackageFullName"),
						version: text("Version"),
						publisher: text("Publisher"),
						install_location: text("InstallLocation"),
						is_framework: false,
						non_removable,
					});
				}
			}
		}
	}

	if result.is_empty() {
		let reg_sub = "Software\\Classes\\Local Settings\\Software\\Microsoft\\Windows\\CurrentVersion\\AppModel\\Repository\\Packages";
		if let Ok(pkg_key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(reg_sub, KEY_READ) {
			for full_name in pkg_key.enum_keys().flatten() {
				let Ok(item_key) = pkg_key.open_subkey_with_flags(&full_name, KEY_READ) else {
					continue;
				};
				let display_name: String = item_key.get_value("DisplayName").unwrap_or_default();
				let package_id: String = item_key.get_value("PackageID").unwrap_or_default();
				let root_folder: String = item_key.get_value("PackageRootFolder").unwrap_or_default();
				let publisher: String = item_key.get_value("Publisher").unwrap_or_default();
				let family = full_name.split('_').next().unwrap_or(&full_name).to_string();
				let clean_name = if !display_name.is_empty() && !display_name.starts_with("@{") {
					display_name
				} else if !package_id.is_empty() {
					package_id
				} else {
					family.clone()
				};
				let lower_family = family.to_lowercase();
				result.push(WindowsStoreApp {
					name: clean_name,
					version: full_name.split('_').nth(1).unwrap_or("").to_string(),
					package_full_name: full_name,
					publisher,
					install_location: root_folder.clone(),
					is_framework: false,
					non_removable: lower_family.starts_with("microsoft.windows.")
						|| lower_family.starts_with("windows.")
						|| root_folder.to_lowercase().contains("\\systemapps\\"),
				});
			}
		}
	}

	result.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
	result.dedup_by(|a, b| a.package_full_name.eq_ignore_ascii_case(&b.package_full_name));
	Ok(result)
}

#[tauri::command(async)]
pub fn remove_windows_store_app(package_full_name: String) -> Result<String, String> {
	let pkg = package_full_name.trim();
	if pkg.is_empty() {
		return Err("PackageFullName cannot be empty.".to_string());
	}
	let script = format!("Remove-AppxPackage -Package {} -ErrorAction Stop", ps_quote(pkg));
	let output = run_powershell(&script)?;
	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
		return Err(format!("Failed to remove `{}`: {}", pkg, stderr));
	}
	Ok(format!("Removed Windows app `{}`.", pkg))
}

// ---------------------------------------------------------------------------
// Startup manager
// ---------------------------------------------------------------------------

/// Task Manager stores the on/off state of startup items here, keyed by value or file name.
/// The original Run value or shortcut is never touched, so disabling is always reversible.
const APPROVED_BASE: &str = "Software\\Microsoft\\Windows\\CurrentVersion\\Explorer\\StartupApproved";

fn approved_enabled(hive: &RegKey, key: &str, value: &str) -> bool {
	hive.open_subkey_with_flags(key, KEY_READ)
		.and_then(|k| k.get_raw_value(value))
		.ok()
		.and_then(|v| v.bytes.first().copied())
		// Even first byte (0x02, 0x06) = enabled, odd (0x03, 0x07) = disabled.
		.map(|b| b & 1 == 0)
		.unwrap_or(true)
}

fn hive_of(label: &str) -> (RegKey, &'static str) {
	if label == "machine" {
		(RegKey::predef(HKEY_LOCAL_MACHINE), "HKLM")
	} else {
		(RegKey::predef(HKEY_CURRENT_USER), "HKCU")
	}
}

#[tauri::command(async)]
pub fn list_startup_entries() -> Result<Vec<StartupEntry>, String> {
	let mut entries: Vec<StartupEntry> = Vec::new();

	let run_keys = [
		("user", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", "Run"),
		("machine", "SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Run", "Run"),
		("machine", "SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Run", "Run32"),
	];
	for (scope, subkey, approved_leaf) in run_keys {
		let (hive, hive_label) = hive_of(scope);
		let Ok(key) = hive.open_subkey_with_flags(subkey, KEY_READ) else {
			continue;
		};
		let approved_key = format!("{}\\{}", APPROVED_BASE, approved_leaf);
		for (name, _) in key.enum_values().flatten() {
			let command: String = key.get_value(&name).unwrap_or_default();
			if command.trim().is_empty() {
				continue;
			}
			let location = format!("{}\\{}", hive_label, subkey);
			entries.push(StartupEntry {
				id: format!("reg:{}::{}", location, name),
				enabled: approved_enabled(&hive, &approved_key, &name),
				approved_value: name.clone(),
				name,
				command,
				location,
				source_kind: "registry-run".to_string(),
				scope: scope.to_string(),
				approved_key: approved_key.clone(),
				file_path: String::new(),
			});
		}
	}

	let folders = [
		("user", env::var("APPDATA").ok(), "Microsoft\\Windows\\Start Menu\\Programs\\Startup"),
		("machine", env::var("ProgramData").ok(), "Microsoft\\Windows\\Start Menu\\Programs\\StartUp"),
	];
	for (scope, base, sub) in folders {
		let Some(base) = base else {
			continue;
		};
		let (hive, _) = hive_of(scope);
		let dir = PathBuf::from(base).join(sub);
		let Ok(items) = fs::read_dir(&dir) else {
			continue;
		};
		let approved_key = format!("{}\\StartupFolder", APPROVED_BASE);
		for item in items.flatten() {
			let file_name = item.file_name().to_string_lossy().to_string();
			if file_name.eq_ignore_ascii_case("desktop.ini") {
				continue;
			}
			// Earlier builds disabled shortcuts by renaming them to `*.disabled`.
			let legacy_disabled = file_name.ends_with(".disabled");
			let clean_name = file_name.trim_end_matches(".disabled").to_string();
			let path = item.path();
			entries.push(StartupEntry {
				id: format!("file:{}", path.display()),
				enabled: !legacy_disabled && approved_enabled(&hive, &approved_key, &clean_name),
				name: clean_name.clone(),
				command: path.display().to_string(),
				location: dir.display().to_string(),
				source_kind: "startup-folder".to_string(),
				scope: scope.to_string(),
				approved_key: approved_key.clone(),
				approved_value: clean_name,
				file_path: path.display().to_string(),
			});
		}
	}

	Ok(entries)
}

#[tauri::command(async)]
pub fn set_startup_entry_enabled(entry: StartupEntry, enable: bool) -> Result<String, String> {
	if !entry.approved_key.starts_with(APPROVED_BASE) {
		return Err("Unexpected startup approval key.".to_string());
	}

	if enable && entry.file_path.ends_with(".disabled") {
		let restored = PathBuf::from(entry.file_path.trim_end_matches(".disabled"));
		fs::rename(&entry.file_path, &restored)
			.map_err(|e| format!("Failed to restore startup shortcut `{}`: {}", entry.name, e))?;
	}

	let (hive, _) = hive_of(&entry.scope);
	let (key, _) = hive.create_subkey(&entry.approved_key).map_err(|e| {
		format!("Cannot open the startup approval key: {}.{}", e, if entry.scope == "machine" { elevation_hint() } else { "" })
	})?;
	let mut bytes = vec![if enable { 0x02 } else { 0x03 }, 0, 0, 0];
	let filetime: u64 = if enable {
		0
	} else {
		// FILETIME counts 100 ns intervals since 1601-01-01.
		(unix_now().as_nanos() / 100) as u64 + 116_444_736_000_000_000
	};
	bytes.extend_from_slice(&filetime.to_le_bytes());
	key.set_raw_value(&entry.approved_value, &RegValue { bytes, vtype: REG_BINARY })
		.map_err(|e| format!("Failed to update `{}`: {}", entry.name, e))?;

	Ok(format!("`{}` will {}start with Windows.", entry.name, if enable { "" } else { "no longer " }))
}

// ---------------------------------------------------------------------------
// Junk cleaner
// ---------------------------------------------------------------------------

struct JunkTarget {
	id: &'static str,
	label: &'static str,
	paths: Vec<PathBuf>,
	process: Option<&'static str>,
}

fn browser_cache_dirs(user_data: &Path) -> Vec<PathBuf> {
	let mut out = Vec::new();
	let Ok(entries) = fs::read_dir(user_data) else {
		return out;
	};
	for entry in entries.flatten() {
		let name = entry.file_name().to_string_lossy().to_string();
		if name == "Default" || name.starts_with("Profile ") || name == "Guest Profile" {
			for sub in ["Cache\\Cache_Data", "Code Cache", "GPUCache"] {
				let p = entry.path().join(sub);
				if p.is_dir() {
					out.push(p);
				}
			}
		}
	}
	out
}

fn junk_targets() -> Vec<JunkTarget> {
	let mut targets = Vec::new();
	targets.push(JunkTarget {
		id: "user-temp",
		label: "Temporary files (%TEMP%)",
		paths: vec![env::temp_dir()],
		process: None,
	});
	if let Ok(root) = env::var("SystemRoot") {
		targets.push(JunkTarget {
			id: "win-temp",
			label: "Windows temporary files",
			paths: vec![PathBuf::from(root).join("Temp")],
			process: None,
		});
	}
	if let Ok(program_data) = env::var("ProgramData") {
		let wer = PathBuf::from(program_data).join("Microsoft\\Windows\\WER");
		targets.push(JunkTarget {
			id: "error-reports",
			label: "Windows error reports",
			paths: vec![wer.join("ReportArchive"), wer.join("ReportQueue")],
			process: None,
		});
	}
	if let Ok(local) = env::var("LOCALAPPDATA") {
		let base = PathBuf::from(local);
		targets.push(JunkTarget {
			id: "crash-dumps",
			label: "Application crash dumps",
			paths: vec![base.join("CrashDumps")],
			process: None,
		});
		targets.push(JunkTarget {
			id: "squirrel-temp",
			label: "Installer staging (SquirrelTemp)",
			paths: vec![base.join("SquirrelTemp")],
			process: None,
		});
		for (id, label, dir, process) in [
			("edge-cache", "Microsoft Edge cache", "Microsoft\\Edge\\User Data", "msedge.exe"),
			("chrome-cache", "Google Chrome cache", "Google\\Chrome\\User Data", "chrome.exe"),
			("brave-cache", "Brave cache", "BraveSoftware\\Brave-Browser\\User Data", "brave.exe"),
		] {
			targets.push(JunkTarget {
				id,
				label,
				paths: browser_cache_dirs(&base.join(dir)),
				process: Some(process),
			});
		}
	}
	targets.retain(|t| t.paths.iter().any(|p| p.is_dir()));
	targets
}

#[tauri::command(async)]
pub fn scan_system_junk() -> Result<Vec<JunkCategoryItem>, String> {
	let running: HashSet<String> = win::snapshot_processes().into_iter().map(|p| p.name.to_lowercase()).collect();
	let mut categories = Vec::new();
	for target in junk_targets() {
		let mut file_count = 0;
		let mut total_bytes = 0u64;
		for path in &target.paths {
			let (count, bytes) = compute_dir_stats(path);
			file_count += count;
			total_bytes = total_bytes.saturating_add(bytes);
		}
		let note = target
			.process
			.filter(|p| running.contains(*p))
			.map(|p| format!("{} is running. Files it has open will be skipped; close it for a full clean.", p));
		categories.push(JunkCategoryItem {
			id: target.id.to_string(),
			category: target.label.to_string(),
			path: target.paths.first().map(|p| p.display().to_string()).unwrap_or_default(),
			file_count,
			total_bytes,
			safe_to_clean: true,
			note,
		});
	}
	Ok(categories)
}

/// Deletes everything inside `dir` file by file. Files that are open elsewhere are counted and skipped.
fn clean_directory_contents(dir: &Path, report: &mut DeletionReport) {
	let Ok(entries) = fs::read_dir(dir) else {
		return;
	};
	for entry in entries.flatten() {
		let path = entry.path();
		let Ok(file_type) = entry.file_type() else {
			continue;
		};
		// Never follow junctions or symlinks out of the cache folder.
		if file_type.is_symlink() {
			continue;
		}
		if file_type.is_dir() {
			clean_directory_contents(&path, report);
			let _ = fs::remove_dir(&path);
			continue;
		}
		let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
		match fs::remove_file(&path) {
			Ok(()) => {
				report.removed_count += 1;
				report.freed_bytes = report.freed_bytes.saturating_add(size);
			}
			// 5 = access denied, 32 = sharing violation, 33 = lock violation.
			Err(e) if matches!(e.raw_os_error(), Some(5) | Some(32) | Some(33)) => report.skipped_in_use += 1,
			Err(e) => {
				if report.errors.len() < 20 {
					report.errors.push(format!("{}: {}", path.display(), e));
				}
			}
		}
	}
}

#[tauri::command(async)]
pub fn clean_system_junk(ids: Vec<String>) -> Result<DeletionReport, String> {
	let wanted: HashSet<String> = ids.into_iter().collect();
	let mut report = DeletionReport::default();
	for target in junk_targets().into_iter().filter(|t| wanted.contains(t.id)) {
		for path in target.paths.iter().filter(|p| p.is_dir()) {
			clean_directory_contents(path, &mut report);
		}
	}
	Ok(report)
}

// ---------------------------------------------------------------------------
// Hunter mode
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn list_active_processes() -> Result<Vec<ActiveProcessItem>, String> {
	let installed = list_installed_apps().unwrap_or_default();
	let mut processes = Vec::new();

	for entry in win::snapshot_processes() {
		if entry.pid <= 4 {
			continue;
		}
		let (path, working_set_bytes) = win::process_details(entry.pid);
		let Some(exe_path) = path else {
			continue;
		};
		let exe_lower = exe_path.to_lowercase();
		let proc_token = normalize_token(entry.name.trim_end_matches(".exe"));
		let matched = installed.iter().find(|app| {
			let loc = app.install_location.trim().trim_end_matches('\\').to_lowercase();
			if loc.len() > 3 && exe_lower.starts_with(&format!("{}\\", loc)) {
				return true;
			}
			let app_token = normalize_token(&clean_display_name(&app.display_name));
			app_token.len() >= 4 && proc_token.len() >= 4 && app_token.contains(&proc_token)
		});

		processes.push(ActiveProcessItem {
			pid: entry.pid,
			name: entry.name,
			exe_path,
			working_set_bytes,
			matched_app_name: matched.map(|a| a.display_name.clone()),
			matched_registry_path: matched.map(|a| a.registry_path.clone()),
		});
	}

	processes.sort_by(|a, b| b.working_set_bytes.cmp(&a.working_set_bytes));
	Ok(processes)
}

#[tauri::command(async)]
pub fn terminate_process_by_pid(pid: u32, force: Option<bool>) -> Result<String, String> {
	let name = win::snapshot_processes()
		.into_iter()
		.find(|p| p.pid == pid)
		.map(|p| p.name)
		.ok_or_else(|| format!("Process {} is no longer running.", pid))?;
	if is_critical_process(&name, pid) {
		return Err(format!("`{}` is a Windows system process and cannot be closed.", name));
	}
	if !force.unwrap_or(false) && win::request_close(pid) > 0 && wait_for_exit(pid, Duration::from_secs(5)) {
		return Ok(format!("Closed `{}` (PID {}).", name, pid));
	}
	let output = hidden_command("taskkill")
		.args(["/F", "/PID", &pid.to_string()])
		.output()
		.map_err(|e| format!("Failed to run taskkill: {}", e))?;
	if !output.status.success() {
		let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
		return Err(format!("Could not end `{}`: {}{}", name, stderr, elevation_hint()));
	}
	Ok(format!("Ended `{}` (PID {}).", name, pid))
}

// ---------------------------------------------------------------------------
// Explorer and icons
// ---------------------------------------------------------------------------

#[tauri::command(async)]
pub fn open_in_explorer(target_path: String) -> Result<String, String> {
	let p = PathBuf::from(target_path.trim().trim_matches('"'));
	if !p.exists() {
		return Err(format!("Path does not exist: {}", p.display()));
	}
	let arg = if p.is_file() {
		format!("/select,\"{}\"", p.display())
	} else {
		format!("\"{}\"", p.display())
	};
	Command::new("explorer")
		.raw_arg(arg)
		.spawn()
		.map_err(|e| format!("Failed to open Explorer: {}", e))?;
	Ok(format!("Opened `{}`.", p.display()))
}

fn base64_encode(bytes: &[u8]) -> String {
	const CHARS: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
	let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
	for chunk in bytes.chunks(3) {
		let b = [chunk[0], *chunk.get(1).unwrap_or(&0), *chunk.get(2).unwrap_or(&0)];
		let triple = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
		out.push(CHARS[((triple >> 18) & 0x3F) as usize] as char);
		out.push(CHARS[((triple >> 12) & 0x3F) as usize] as char);
		out.push(if chunk.len() > 1 { CHARS[((triple >> 6) & 0x3F) as usize] as char } else { '=' });
		out.push(if chunk.len() > 2 { CHARS[(triple & 0x3F) as usize] as char } else { '=' });
	}
	out
}

/// `"C:\x\app.exe",0` → `C:\x\app.exe`
fn icon_file(raw: &str) -> Option<PathBuf> {
	let clean = expand_env(raw.trim().trim_matches('"'));
	let before_comma = clean.split(',').next().unwrap_or("").trim().trim_matches('"').to_string();
	if before_comma.is_empty() {
		return None;
	}
	let p = PathBuf::from(before_comma);
	p.is_file().then_some(p)
}

/// Resolves many `DisplayIcon` values at once with a single PowerShell process.
#[tauri::command(async)]
pub fn get_app_icons(raw_paths: Vec<String>) -> Result<HashMap<String, String>, String> {
	let mut icons: HashMap<String, String> = HashMap::new();
	let mut to_extract: Vec<(String, PathBuf)> = Vec::new();

	for raw in raw_paths {
		let Some(file) = icon_file(&raw) else {
			continue;
		};
		let ext = file.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
		let mime = match ext.as_str() {
			"ico" => Some("image/x-icon"),
			"png" => Some("image/png"),
			"jpg" | "jpeg" => Some("image/jpeg"),
			_ => None,
		};
		match mime {
			Some(mime) => {
				if let Ok(bytes) = fs::read(&file) {
					if bytes.len() < 2 * 1024 * 1024 {
						icons.insert(raw, format!("data:{};base64,{}", mime, base64_encode(&bytes)));
					}
				}
			}
			None if matches!(ext.as_str(), "exe" | "dll") => to_extract.push((raw, file)),
			None => {}
		}
	}

	if to_extract.is_empty() {
		return Ok(icons);
	}

	let list_file = env::temp_dir().join(format!("tuntas-icons-{}.txt", std::process::id()));
	let list: String = to_extract.iter().map(|(_, p)| format!("{}\n", p.display())).collect();
	fs::write(&list_file, list).map_err(|e| format!("Cannot write icon list: {}", e))?;
	let script = format!(
		"[Console]::OutputEncoding = [System.Text.Encoding]::UTF8; Add-Type -AssemblyName System.Drawing; $i = 0; foreach ($p in Get-Content -LiteralPath {} -Encoding UTF8) {{ try {{ $icon = [System.Drawing.Icon]::ExtractAssociatedIcon($p); if ($icon) {{ $ms = New-Object System.IO.MemoryStream; $bmp = $icon.ToBitmap(); $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png); Write-Output (\"$i`t\" + [Convert]::ToBase64String($ms.ToArray())); $bmp.Dispose(); $icon.Dispose(); $ms.Dispose() }} }} catch {{}}; $i++ }}",
		ps_quote(&list_file.display().to_string())
	);
	let output = run_powershell(&script);
	let _ = fs::remove_file(&list_file);
	let output = output?;
	for line in String::from_utf8_lossy(&output.stdout).lines() {
		let Some((index, b64)) = line.split_once('\t') else {
			continue;
		};
		if let Some((raw, _)) = index.trim().parse::<usize>().ok().and_then(|i| to_extract.get(i)) {
			icons.insert(raw.clone(), format!("data:image/png;base64,{}", b64.trim()));
		}
	}
	Ok(icons)
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn cleans_display_names() {
		assert_eq!(clean_display_name("7-Zip 23.01 (x64)"), "7-Zip");
		assert_eq!(clean_display_name("Python 3.12.1 (64-bit)"), "Python");
		assert_eq!(clean_display_name("Mozilla Firefox (x64 en-US)"), "Mozilla Firefox");
		assert_eq!(clean_display_name("Notepad++ v8.6.4"), "Notepad++");
	}

	#[test]
	fn shared_folders_never_match_by_containment() {
		let teams = AppTokens::new("Microsoft Teams", "Microsoft Corporation");
		assert!(!teams.matches_top_level("microsoft"));
		assert!(teams.is_publisher("microsoft"));
		assert!(teams.matches_under_publisher("teams"));

		let chrome = AppTokens::new("Google Chrome", "Google LLC");
		assert!(!chrome.matches_top_level("google"));
		assert!(chrome.matches_under_publisher("chrome"));

		let runtime = AppTokens::new("Microsoft Windows Desktop Runtime - 8.0.4 (x64)", "Microsoft Corporation");
		assert!(!runtime.matches_under_publisher("windows"));
	}

	#[test]
	fn splits_command_lines() {
		assert_eq!(
			split_command_line("\"C:\\Program Files\\App\\unins000.exe\" /SILENT"),
			("C:\\Program Files\\App\\unins000.exe".to_string(), "/SILENT".to_string())
		);
		assert_eq!(
			split_command_line("MsiExec.exe /I{1234}"),
			("MsiExec.exe".to_string(), "/I{1234}".to_string())
		);
	}

	#[test]
	fn turns_msi_install_into_uninstall() {
		assert_eq!(fix_msi_arguments("/I{ABC}", false), "/X{ABC}");
		assert_eq!(fix_msi_arguments("/i{ABC}", true), "/X{ABC} /qb /norestart");
		assert_eq!(fix_msi_arguments("/X{ABC} /qn", true), "/X{ABC} /qn");
	}

	#[test]
	fn guards_registry_deletes() {
		assert!(check_registry_deletable("HKLM\\SOFTWARE").is_err());
		assert!(check_registry_deletable("HKLM\\SOFTWARE\\Microsoft").is_err());
		assert!(check_registry_deletable("HKCU\\SOFTWARE\\Classes").is_err());
		assert!(check_registry_deletable("HKCU\\SYSTEM\\Foo").is_err());
		assert!(check_registry_deletable("HKCU\\SOFTWARE\\Discord").is_ok());
		assert!(check_registry_deletable("HKCU\\SOFTWARE\\Microsoft\\Teams").is_ok());
		assert!(check_registry_deletable("HKLM\\SOFTWARE\\Microsoft\\Windows NT").is_err());
		assert!(check_registry_deletable("HKLM\\SOFTWARE\\Microsoft\\Windows").is_err());
		assert!(check_registry_deletable("HKLM\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{X}").is_ok());
		assert!(check_registry_deletable("HKLM\\SOFTWARE\\WOW6432Node\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\X").is_ok());
	}

	#[test]
	fn expands_environment_variables() {
		std::env::set_var("TUNTAS_TEST_VAR", "C:\\X");
		assert_eq!(expand_env("%TUNTAS_TEST_VAR%\\a.exe"), "C:\\X\\a.exe");
		assert_eq!(expand_env("%NOPE_NOT_SET%\\a"), "%NOPE_NOT_SET%\\a");
		assert_eq!(expand_env("100%"), "100%");
	}
}
