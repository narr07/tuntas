import { invoke } from '@tauri-apps/api/core';

export interface InstalledApp {
	id: string;
	displayName: string;
	displayVersion: string;
	publisher: string;
	installDate: string;
	installLocation: string;
	estimatedSizeKb: number;
	uninstallString: string;
	quietUninstallString: string;
	displayIcon: string;
	registryPath: string;
	architecture: string;
	systemComponent: boolean;
}

export type LeftoverConfidence = 'exact-path' | 'registered-path' | 'publisher-match' | 'name-match';

export interface LeftoverItem {
	id: string;
	kind: 'folder' | 'file' | 'registry';
	path: string;
	confidence: LeftoverConfidence;
	sizeBytes: number;
	reason: string;
}

export interface LockingProcess {
	pid: number;
	name: string;
	appName: string;
	serviceName: string;
}

export interface LockedItem {
	path: string;
	files: string[];
	processes: LockingProcess[];
}

export interface DeletionReport {
	removedCount: number;
	freedBytes: number;
	skippedInUse: number;
	errors: string[];
	locked: LockedItem[];
	backupDir: string | null;
}

export interface ReleaseReport {
	released: number[];
	stillRunning: LockingProcess[];
	errors: string[];
}

export interface ScheduleReport {
	scheduled: number;
	errors: string[];
}

export interface NativeUninstallResult {
	pid: number;
	command: string;
	exitCode: number | null;
	success: boolean;
	stillInstalled: boolean;
	message: string;
}

export interface WindowsStoreApp {
	name: string;
	packageFullName: string;
	version: string;
	publisher: string;
	installLocation: string;
	isFramework: boolean;
	nonRemovable: boolean;
}

export interface StartupEntry {
	id: string;
	name: string;
	command: string;
	location: string;
	sourceKind: 'registry-run' | 'startup-folder';
	scope: 'user' | 'machine';
	enabled: boolean;
	approvedKey: string;
	approvedValue: string;
	filePath: string;
}

export interface JunkCategoryItem {
	id: string;
	category: string;
	path: string;
	fileCount: number;
	totalBytes: number;
	safeToClean: boolean;
	note: string | null;
}

export interface ActiveProcessItem {
	pid: number;
	name: string;
	exePath: string;
	workingSetBytes: number;
	matchedAppName: string | null;
	matchedRegistryPath: string | null;
}

export function formatBytes(bytes: number): string {
	if (!Number.isFinite(bytes) || bytes <= 0) {
		return '0 B';
	}
	const units = ['B', 'KB', 'MB', 'GB', 'TB'];
	const index = Math.min(Math.floor(Math.log(bytes) / Math.log(1024)), units.length - 1);
	const value = bytes / Math.pow(1024, index);
	return `${value.toFixed(index === 0 ? 0 : 1)} ${units[index]}`;
}

export function formatKb(kb: number): string {
	if (!Number.isFinite(kb) || kb <= 0) {
		return '-';
	}
	return formatBytes(kb * 1024);
}

export function formatInstallDate(raw: string): string {
	const trimmed = raw.trim();
	if (/^\d{8}$/.test(trimmed)) {
		const year = trimmed.slice(0, 4);
		const month = trimmed.slice(4, 6);
		const day = trimmed.slice(6, 8);
		return `${year}-${month}-${day}`;
	}
	return trimmed || '-';
}

export function errorText(err: unknown): string {
	return err instanceof Error ? err.message : String(err);
}

/** Leftovers that are safe to tick by default. Registry keys only when they are the program's own uninstall entry. */
export function isPreselectedLeftover(item: LeftoverItem): boolean {
	if (item.kind === 'registry') {
		return item.confidence === 'exact-path';
	}
	return item.confidence !== 'registered-path';
}

/** Leftovers the user should double-check before deleting. */
export function isRiskyLeftover(item: LeftoverItem): boolean {
	return item.confidence === 'registered-path' || (item.kind === 'registry' && item.confidence !== 'exact-path');
}

export const confidenceLabel: Record<LeftoverConfidence, string> = {
	'exact-path': 'Exact path',
	'registered-path': 'Registered path, verify',
	'publisher-match': 'Publisher folder',
	'name-match': 'Same name'
};

export function useUninstallerBackend() {
	const isTauriRuntime = computed(() => {
		if (!import.meta.client) {
			return false;
		}
		return '__TAURI_INTERNALS__' in window;
	});

	function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
		if (!isTauriRuntime.value) {
			return Promise.reject(new Error('Tuntas is running in a browser tab. Start it with `bun run desktop:dev` to reach Windows.'));
		}
		return invoke<T>(command, args);
	}

	return {
		isTauriRuntime,
		isElevated: () => call<boolean>('is_elevated'),
		restartAsAdmin: () => call<null>('restart_as_admin'),
		createRestorePoint: (description: string) => call<string>('create_restore_point', { description }),
		openBackupFolder: () => call<string>('open_backup_folder'),
		listInstalledApps: () => call<InstalledApp[]>('list_installed_apps'),
		runNativeUninstaller: (app: InstalledApp, preferQuiet: boolean) => call<NativeUninstallResult>('run_native_uninstaller', {
			uninstallString: app.uninstallString,
			quietUninstallString: app.quietUninstallString,
			preferQuiet,
			registryPath: app.registryPath
		}),
		scanAppLeftovers: (app: InstalledApp) => call<LeftoverItem[]>('scan_app_leftovers', {
			displayName: app.displayName,
			publisher: app.publisher,
			installLocation: app.installLocation,
			registryPath: app.registryPath,
			uninstallString: app.uninstallString
		}),
		forcedUninstallScan: (targetName: string, publisher: string, targetPath: string, registryPath?: string) => call<LeftoverItem[]>('forced_uninstall_scan', {
			targetName,
			publisher,
			targetPath,
			registryPath: registryPath || null
		}),
		deleteLeftoverItems: (items: LeftoverItem[], permanent = false) => call<DeletionReport>('delete_leftover_items', { items, permanent }),
		releaseLocks: (processes: LockingProcess[], force: boolean) => call<ReleaseReport>('release_locks', { processes, force }),
		scheduleDeleteOnReboot: (paths: string[]) => call<ScheduleReport>('schedule_delete_on_reboot', { paths }),
		listWindowsStoreApps: () => call<WindowsStoreApp[]>('list_windows_store_apps'),
		removeWindowsStoreApp: (packageFullName: string) => call<string>('remove_windows_store_app', { packageFullName }),
		listStartupEntries: () => call<StartupEntry[]>('list_startup_entries'),
		setStartupEntryEnabled: (entry: StartupEntry, enable: boolean) => call<string>('set_startup_entry_enabled', { entry, enable }),
		scanSystemJunk: () => call<JunkCategoryItem[]>('scan_system_junk'),
		cleanSystemJunk: (ids: string[]) => call<DeletionReport>('clean_system_junk', { ids }),
		listActiveProcesses: () => call<ActiveProcessItem[]>('list_active_processes'),
		terminateProcessByPid: (pid: number, force = false) => call<string>('terminate_process_by_pid', { pid, force }),
		openInExplorer: (targetPath: string) => call<string>('open_in_explorer', { targetPath }),
		getAppIcons: (rawPaths: string[]) => call<Record<string, string>>('get_app_icons', { rawPaths })
	};
}

/** Program icons keyed by their `DisplayIcon` value, fetched in batches through one PowerShell call each. */
export function useAppIcons() {
	const icons = useState<Record<string, string>>('tuntas-icons', () => ({}));
	const { getAppIcons, isTauriRuntime } = useUninstallerBackend();

	async function load(apps: InstalledApp[]) {
		if (!isTauriRuntime.value) {
			return;
		}
		const missing = [...new Set(apps.map(a => a.displayIcon).filter(p => p && !(p in icons.value)))];
		for (const path of missing) {
			icons.value[path] = '';
		}
		for (let i = 0; i < missing.length; i += 60) {
			try {
				const found = await getAppIcons(missing.slice(i, i + 60));
				icons.value = { ...icons.value, ...found };
			}
			catch {
				// Avatars fall back to the program initials.
			}
		}
	}

	function iconFor(app: InstalledApp): string | undefined {
		return icons.value[app.displayIcon] || undefined;
	}

	return { load, iconFor };
}

/** Shared admin-rights state, read once per session. */
export function useElevation() {
	const elevated = useState<boolean | null>('tuntas-elevated', () => null);
	const { isElevated, isTauriRuntime } = useUninstallerBackend();

	async function refresh() {
		if (!isTauriRuntime.value) {
			return;
		}
		try {
			elevated.value = await isElevated();
		}
		catch {
			elevated.value = false;
		}
	}

	return { elevated, refresh };
}
