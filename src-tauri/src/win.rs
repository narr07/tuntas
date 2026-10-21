//! Thin wrappers around the Win32 APIs Tuntas needs. Everything `unsafe` lives here.

use serde::{Deserialize, Serialize};
use std::ffi::OsStr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::process::CommandExt;
use std::path::Path;
use std::process::Command;

use windows_sys::Win32::Foundation::{CloseHandle, ERROR_MORE_DATA, HANDLE, HWND, INVALID_HANDLE_VALUE, LPARAM};
use windows_sys::Win32::Security::{GetTokenInformation, TokenElevation, TOKEN_ELEVATION, TOKEN_QUERY};
use windows_sys::Win32::Storage::FileSystem::{MoveFileExW, MOVEFILE_DELAY_UNTIL_REBOOT};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
	CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W, TH32CS_SNAPPROCESS,
};
use windows_sys::Win32::System::ProcessStatus::{K32GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
use windows_sys::Win32::System::RestartManager::{
	RmEndSession, RmGetList, RmRegisterResources, RmStartSession, CCH_RM_SESSION_KEY, RM_PROCESS_INFO,
};
use windows_sys::Win32::System::Threading::{
	GetCurrentProcess, OpenProcess, OpenProcessToken, QueryFullProcessImageNameW, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows_sys::Win32::UI::Shell::{
	SHFileOperationW, ShellExecuteW, FOF_ALLOWUNDO, FOF_NOCONFIRMATION, FOF_NOERRORUI, FOF_SILENT, FO_DELETE,
	SHFILEOPSTRUCTW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{
	EnumWindows, GetWindowThreadProcessId, IsWindowVisible, PostMessageW, SW_SHOWNORMAL, WM_CLOSE,
};

const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// A `Command` that never flashes a console window (powershell, reg, taskkill, ...).
pub fn hidden_command<S: AsRef<OsStr>>(program: S) -> Command {
	let mut cmd = Command::new(program);
	cmd.creation_flags(CREATE_NO_WINDOW);
	cmd
}

pub fn wide(s: &OsStr) -> Vec<u16> {
	s.encode_wide().chain(std::iter::once(0)).collect()
}

fn from_wide(buf: &[u16]) -> String {
	let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
	String::from_utf16_lossy(&buf[..len])
}

struct OwnedHandle(HANDLE);

impl Drop for OwnedHandle {
	fn drop(&mut self) {
		if !self.0.is_null() && self.0 != INVALID_HANDLE_VALUE {
			unsafe {
				CloseHandle(self.0);
			}
		}
	}
}

#[derive(Debug, Clone)]
pub struct ProcessEntry {
	pub pid: u32,
	pub parent_pid: u32,
	pub name: String,
}

pub fn snapshot_processes() -> Vec<ProcessEntry> {
	let mut out = Vec::new();
	unsafe {
		let snap = OwnedHandle(CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0));
		if snap.0 == INVALID_HANDLE_VALUE {
			return out;
		}
		let mut entry: PROCESSENTRY32W = std::mem::zeroed();
		entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
		if Process32FirstW(snap.0, &mut entry) == 0 {
			return out;
		}
		loop {
			out.push(ProcessEntry {
				pid: entry.th32ProcessID,
				parent_pid: entry.th32ParentProcessID,
				name: from_wide(&entry.szExeFile),
			});
			if Process32NextW(snap.0, &mut entry) == 0 {
				break;
			}
		}
	}
	out
}

/// Full executable path and working set of a process, when the caller is allowed to query it.
pub fn process_details(pid: u32) -> (Option<String>, u64) {
	unsafe {
		let handle = OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
		if handle.0.is_null() {
			return (None, 0);
		}
		let mut buf = [0u16; 1024];
		let mut size = buf.len() as u32;
		let path = if QueryFullProcessImageNameW(handle.0, 0, buf.as_mut_ptr(), &mut size) != 0 {
			Some(String::from_utf16_lossy(&buf[..size as usize]))
		} else {
			None
		};
		let mut counters: PROCESS_MEMORY_COUNTERS = std::mem::zeroed();
		counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
		let working_set = if K32GetProcessMemoryInfo(handle.0, &mut counters, counters.cb) != 0 {
			counters.WorkingSetSize as u64
		} else {
			0
		};
		(path, working_set)
	}
}

pub fn is_elevated() -> bool {
	unsafe {
		let mut token: HANDLE = std::ptr::null_mut();
		if OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut token) == 0 {
			return false;
		}
		let token = OwnedHandle(token);
		let mut elevation: TOKEN_ELEVATION = std::mem::zeroed();
		let mut returned = 0u32;
		let ok = GetTokenInformation(
			token.0,
			TokenElevation,
			&mut elevation as *mut _ as *mut core::ffi::c_void,
			std::mem::size_of::<TOKEN_ELEVATION>() as u32,
			&mut returned,
		);
		ok != 0 && elevation.TokenIsElevated != 0
	}
}

/// Starts a new, elevated copy of the current executable through the UAC prompt.
pub fn relaunch_elevated() -> Result<(), String> {
	let exe = std::env::current_exe().map_err(|e| format!("Cannot locate the Tuntas executable: {}", e))?;
	let verb = wide(OsStr::new("runas"));
	let file = wide(exe.as_os_str());
	let result = unsafe {
		ShellExecuteW(
			std::ptr::null_mut(),
			verb.as_ptr(),
			file.as_ptr(),
			std::ptr::null(),
			std::ptr::null(),
			SW_SHOWNORMAL,
		)
	};
	// ShellExecuteW returns a value greater than 32 on success.
	if (result as isize) > 32 {
		Ok(())
	} else {
		Err("Administrator rights were not granted.".to_string())
	}
}

/// Moves a file or folder to the Recycle Bin without any shell dialog.
pub fn recycle(path: &Path) -> Result<(), String> {
	// pFrom must be double-null terminated.
	let mut from = wide(path.as_os_str());
	from.push(0);
	let mut op: SHFILEOPSTRUCTW = unsafe { std::mem::zeroed() };
	op.wFunc = FO_DELETE;
	op.pFrom = from.as_ptr();
	op.fFlags = (FOF_ALLOWUNDO | FOF_NOCONFIRMATION | FOF_NOERRORUI | FOF_SILENT) as u16;
	let code = unsafe { SHFileOperationW(&mut op) };
	if code != 0 {
		return Err(format!("Windows shell error 0x{:X}", code));
	}
	if op.fAnyOperationsAborted != 0 || path.exists() {
		return Err("Some files could not be moved to the Recycle Bin".to_string());
	}
	Ok(())
}

/// Registers `path` for deletion during the next boot (needs administrator rights).
pub fn delete_on_reboot(path: &Path) -> Result<(), String> {
	let src = wide(path.as_os_str());
	let ok = unsafe { MoveFileExW(src.as_ptr(), std::ptr::null(), MOVEFILE_DELAY_UNTIL_REBOOT) };
	if ok == 0 {
		return Err(format!(
			"Could not schedule `{}` for deletion: {}",
			path.display(),
			std::io::Error::last_os_error()
		));
	}
	Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LockingProcess {
	pub pid: u32,
	pub name: String,
	pub app_name: String,
	pub service_name: String,
}

/// Asks the Restart Manager which processes hold any of `files` open.
pub fn find_lockers(files: &[std::path::PathBuf]) -> Vec<LockingProcess> {
	if files.is_empty() {
		return Vec::new();
	}
	let mut result = Vec::new();
	unsafe {
		let mut session = 0u32;
		let mut key = [0u16; CCH_RM_SESSION_KEY as usize + 1];
		if RmStartSession(&mut session, 0, key.as_mut_ptr()) != 0 {
			return result;
		}
		let wide_files: Vec<Vec<u16>> = files.iter().map(|f| wide(f.as_os_str())).collect();
		let ptrs: Vec<*const u16> = wide_files.iter().map(|w| w.as_ptr()).collect();
		if RmRegisterResources(
			session,
			ptrs.len() as u32,
			ptrs.as_ptr(),
			0,
			std::ptr::null(),
			0,
			std::ptr::null(),
		) == 0
		{
			let mut needed = 0u32;
			let mut count = 0u32;
			let mut reasons = 0u32;
			let first = RmGetList(session, &mut needed, &mut count, std::ptr::null_mut(), &mut reasons);
			if first == ERROR_MORE_DATA && needed > 0 {
				let mut infos: Vec<RM_PROCESS_INFO> = vec![std::mem::zeroed(); needed as usize];
				count = needed;
				if RmGetList(session, &mut needed, &mut count, infos.as_mut_ptr(), &mut reasons) == 0 {
					let procs = snapshot_processes();
					for info in infos.iter().take(count as usize) {
						let pid = info.Process.dwProcessId;
						let name = procs
							.iter()
							.find(|p| p.pid == pid)
							.map(|p| p.name.clone())
							.unwrap_or_default();
						result.push(LockingProcess {
							pid,
							name,
							app_name: from_wide(&info.strAppName),
							service_name: from_wide(&info.strServiceShortName),
						});
					}
				}
			}
		}
		RmEndSession(session);
	}
	result.dedup_by(|a, b| a.pid == b.pid);
	result
}

struct CloseRequest {
	pid: u32,
	posted: u32,
}

unsafe extern "system" fn close_window_callback(hwnd: HWND, lparam: LPARAM) -> windows_sys::core::BOOL {
	let request = &mut *(lparam as *mut CloseRequest);
	let mut owner = 0u32;
	GetWindowThreadProcessId(hwnd, &mut owner);
	if owner == request.pid && IsWindowVisible(hwnd) != 0 {
		PostMessageW(hwnd, WM_CLOSE, 0, 0);
		request.posted += 1;
	}
	1
}

/// Politely asks a process to exit by closing its visible top-level windows.
/// Returns how many windows were asked to close.
pub fn request_close(pid: u32) -> u32 {
	let mut request = CloseRequest { pid, posted: 0 };
	unsafe {
		EnumWindows(Some(close_window_callback), &mut request as *mut CloseRequest as LPARAM);
	}
	request.posted
}

pub fn is_running(pid: u32) -> bool {
	snapshot_processes().iter().any(|p| p.pid == pid)
}
