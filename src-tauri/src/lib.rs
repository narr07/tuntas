#[cfg(not(windows))]
compile_error!("Tuntas only targets Windows.");

pub mod commands;
mod safety;
mod win;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
	tauri::Builder::default()
		.setup(|app| {
			if cfg!(debug_assertions) {
				app.handle().plugin(
					tauri_plugin_log::Builder::default()
						.level(log::LevelFilter::Info)
						.build(),
				)?;
			}
			Ok(())
		})
		.invoke_handler(tauri::generate_handler![
			commands::is_elevated,
			commands::restart_as_admin,
			commands::create_restore_point,
			commands::open_backup_folder,
			commands::list_installed_apps,
			commands::run_native_uninstaller,
			commands::scan_app_leftovers,
			commands::forced_uninstall_scan,
			commands::delete_leftover_items,
			commands::release_locks,
			commands::schedule_delete_on_reboot,
			commands::list_windows_store_apps,
			commands::remove_windows_store_app,
			commands::list_startup_entries,
			commands::set_startup_entry_enabled,
			commands::scan_system_junk,
			commands::clean_system_junk,
			commands::list_active_processes,
			commands::terminate_process_by_pid,
			commands::open_in_explorer,
			commands::get_app_icons
		])
		.run(tauri::generate_context!())
		.expect("error while running tauri application");
}
