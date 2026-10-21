<script setup lang="ts">
import type { TableColumn, TableRow } from '@nuxt/ui';
import type { DeletionReport, InstalledApp, LeftoverItem, LockedItem } from '~/composables/useUninstallerBackend';
import {
	errorText,
	formatBytes,
	formatInstallDate,
	formatKb,
	isPreselectedLeftover,
	isRiskyLeftover,
	useAppIcons,
	useElevation,
	useUninstallerBackend
} from '~/composables/useUninstallerBackend';
import { breakpointsTailwind, useBreakpoints } from '@vueuse/core';
import { h, resolveComponent } from 'vue';

type Banner = {
	color: 'success' | 'error' | 'info' | 'warning';
	title: string;
	description?: string;
	backupDir?: string | null;
};

const UCheckbox = resolveComponent('UCheckbox');
const UAvatar = resolveComponent('UAvatar');
const UBadge = resolveComponent('UBadge');
const toast = useToast();

const {
	listInstalledApps,
	runNativeUninstaller,
	scanAppLeftovers,
	deleteLeftoverItems,
	openInExplorer,
	openBackupFolder,
	createRestorePoint
} = useUninstallerBackend();
const { load: loadIcons, iconFor } = useAppIcons();
const { elevated } = useElevation();

const breakpoints = useBreakpoints(breakpointsTailwind);
const isMobile = breakpoints.smaller('lg');

const apps = ref<InstalledApp[]>([]);
const isLoading = ref(false);
const errorMessage = ref<string | null>(null);
const banner = ref<Banner | null>(null);

const searchQuery = ref('');
const archFilter = ref<'all' | 'x64' | 'x86' | 'User'>('all');
const showSystemComponents = ref(false);
const sortBy = ref<'name' | 'size' | 'date'>('name');

const selectedApp = ref<InstalledApp | null>(null);
const isNativeUninstalling = ref(false);
const isScanningLeftovers = ref(false);
const leftovers = ref<LeftoverItem[]>([]);
const selectedLeftoverIds = ref<Set<string>>(new Set());
const hasScannedCurrentApp = ref(false);
const hasCompletedNativeUninstall = ref(false);
const isDeletingLeftovers = ref(false);

// Keys are app ids because the table uses `get-row-id`.
const rowSelection = ref<Record<string, boolean>>({});

const isBatchUninstalling = ref(false);
const batchProgress = ref<{ current: number; total: number; currentName: string } | null>(null);
const isBatchCancelRequested = ref(false);

// Confirmation Modals State
const confirmDeleteLeftoversOpen = ref(false);
const confirmBatchUninstallOpen = ref(false);
const confirmSingleUninstallOpen = ref(false);
const appToUninstall = ref<{ app: InstalledApp; preferQuiet: boolean } | null>(null);
const makeRestorePoint = ref(true);

const lockOpen = ref(false);
const lockedItems = ref<LockedItem[]>([]);

const inspectorOpen = computed({
	get: () => Boolean(selectedApp.value),
	set: (value: boolean) => {
		if (!value) {
			selectedApp.value = null;
		}
	}
});

const filteredApps = computed(() => {
	const q = searchQuery.value.trim().toLowerCase();
	const list = apps.value.filter((app) => {
		if (!showSystemComponents.value && app.systemComponent) {
			return false;
		}
		if (archFilter.value !== 'all' && app.architecture !== archFilter.value) {
			return false;
		}
		if (!q) {
			return true;
		}
		return (
			app.displayName.toLowerCase().includes(q)
			|| app.publisher.toLowerCase().includes(q)
			|| app.installLocation.toLowerCase().includes(q)
		);
	});

	return list.sort((a, b) => {
		if (sortBy.value === 'size') {
			return b.estimatedSizeKb - a.estimatedSizeKb;
		}
		if (sortBy.value === 'date') {
			return b.installDate.localeCompare(a.installDate);
		}
		return a.displayName.localeCompare(b.displayName);
	});
});

const selectedBatchApps = computed(() => apps.value.filter(a => rowSelection.value[a.id]));
const selectedBatchSizeKb = computed(() => selectedBatchApps.value.reduce((acc, a) => acc + (a.estimatedSizeKb || 0), 0));
const totalDiskSizeKb = computed(() => apps.value.reduce((acc, a) => acc + (a.estimatedSizeKb || 0), 0));
const x64AppsCount = computed(() => apps.value.filter(a => a.architecture === 'x64').length);
const hasActiveFilter = computed(() => Boolean(searchQuery.value.trim()) || archFilter.value !== 'all');

const leftoversToDelete = computed(() => leftovers.value.filter(item => selectedLeftoverIds.value.has(item.id)));
const riskyLeftoversToDelete = computed(() => leftoversToDelete.value.filter(isRiskyLeftover));

const columns: TableColumn<InstalledApp>[] = [
	{
		id: 'select',
		header: ({ table }) => h(UCheckbox, {
			'modelValue': table.getIsSomePageRowsSelected() ? 'indeterminate' : table.getIsAllPageRowsSelected(),
			'onUpdate:modelValue': (v: boolean | 'indeterminate') => table.toggleAllPageRowsSelected(Boolean(v)),
			'aria-label': 'Select all programs'
		}),
		cell: ({ row }) => h(UCheckbox, {
			'modelValue': row.getIsSelected(),
			'onUpdate:modelValue': (v: boolean | 'indeterminate') => row.toggleSelected(Boolean(v)),
			'onClick': (e: Event) => e.stopPropagation(),
			'aria-label': `Select ${row.original.displayName}`
		}),
		meta: { class: { th: 'w-10', td: 'w-10' } }
	},
	{
		accessorKey: 'displayName',
		header: 'Program',
		cell: ({ row }) => {
			const app = row.original;
			return h('div', { class: 'flex items-center gap-3 min-w-0' }, [
				h(UAvatar, {
					src: iconFor(app),
					alt: app.displayName,
					size: 'md',
					class: 'rounded-md bg-elevated ring ring-default shrink-0'
				}),
				h('div', { class: 'min-w-0' }, [
					h('div', { class: 'flex items-center gap-1.5 min-w-0' }, [
						h('p', { class: 'font-medium text-highlighted truncate' }, app.displayName),
						h(UBadge, { label: app.architecture, color: 'neutral', variant: 'subtle', size: 'sm', class: 'shrink-0' })
					]),
					h('p', { class: 'text-xs text-muted truncate' }, [
						app.publisher || 'Unknown publisher',
						app.displayVersion ? `, v${app.displayVersion}` : ''
					].join(''))
				])
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'estimatedSizeKb',
		header: 'Size',
		cell: ({ row }) => formatKb(row.original.estimatedSizeKb),
		meta: { class: { th: 'text-right', td: 'text-right tabular-nums text-muted whitespace-nowrap' } }
	},
	{
		accessorKey: 'installDate',
		header: 'Installed',
		cell: ({ row }) => formatInstallDate(row.original.installDate),
		meta: { class: { th: 'hidden 2xl:table-cell', td: 'hidden 2xl:table-cell tabular-nums text-muted whitespace-nowrap' } }
	}
];

const tableMeta = {
	class: {
		tr: (row: TableRow<InstalledApp>) => row.original.id === selectedApp.value?.id ? 'bg-elevated' : ''
	}
};

async function refreshApps() {
	isLoading.value = true;
	errorMessage.value = null;
	try {
		const result = await listInstalledApps();
		apps.value = result;
		if (selectedApp.value) {
			// Keep the inspector open on an app that was just removed so its leftovers stay visible.
			selectedApp.value = result.find(a => a.id === selectedApp.value?.id) || selectedApp.value;
		}
		const ids = new Set(result.map(a => a.id));
		rowSelection.value = Object.fromEntries(Object.entries(rowSelection.value).filter(([id]) => ids.has(id)));
		loadIcons(result);
	}
	catch (err: unknown) {
		errorMessage.value = errorText(err);
	}
	finally {
		isLoading.value = false;
	}
}

function selectProgram(app: InstalledApp) {
	if (selectedApp.value?.id === app.id) {
		return;
	}
	selectedApp.value = app;
	leftovers.value = [];
	selectedLeftoverIds.value = new Set();
	hasScannedCurrentApp.value = false;
	hasCompletedNativeUninstall.value = false;
}

function resetFilters() {
	searchQuery.value = '';
	archFilter.value = 'all';
}

function requestSingleUninstall(app: InstalledApp, preferQuiet: boolean) {
	appToUninstall.value = { app, preferQuiet };
	confirmSingleUninstallOpen.value = true;
}

async function executeSingleUninstall() {
	if (!appToUninstall.value) {
		return;
	}
	const { app, preferQuiet } = appToUninstall.value;
	confirmSingleUninstallOpen.value = false;
	await triggerUninstall(app, preferQuiet);
	appToUninstall.value = null;
}

async function maybeCreateRestorePoint(description: string) {
	if (!makeRestorePoint.value || !elevated.value) {
		return;
	}
	try {
		const message = await createRestorePoint(description);
		toast.add({ title: 'Restore point', description: message, color: 'info', icon: 'i-lucide-history' });
	}
	catch (err: unknown) {
		toast.add({ title: 'No restore point', description: errorText(err), color: 'warning', icon: 'i-lucide-history' });
	}
}

async function triggerUninstall(app: InstalledApp, preferQuiet: boolean) {
	banner.value = null;
	isNativeUninstalling.value = true;
	try {
		await maybeCreateRestorePoint(`Before removing ${app.displayName}`);
		const res = await runNativeUninstaller(app, preferQuiet);
		hasCompletedNativeUninstall.value = true;
		if (res.stillInstalled) {
			banner.value = {
				color: 'warning',
				title: `${app.displayName} is still installed`,
				description: `${res.message} Leftovers were not scanned, so nothing the program still needs gets deleted. Run the uninstaller again, or use Scan again if you are sure it is gone.`
			};
			return;
		}
		toast.add({ title: 'Uninstaller finished', description: `${res.message} Looking for leftovers now.`, color: 'info' });
		await triggerLeftoverScan(app);
		await refreshApps();
	}
	catch (err: unknown) {
		banner.value = { color: 'error', title: `Could not uninstall ${app.displayName}`, description: errorText(err) };
	}
	finally {
		isNativeUninstalling.value = false;
	}
}

async function triggerLeftoverScan(app: InstalledApp) {
	isScanningLeftovers.value = true;
	hasScannedCurrentApp.value = false;
	try {
		const found = await scanAppLeftovers(app);
		leftovers.value = found;
		selectedLeftoverIds.value = new Set(found.filter(isPreselectedLeftover).map(item => item.id));
		hasScannedCurrentApp.value = true;
	}
	catch (err: unknown) {
		banner.value = { color: 'error', title: 'Leftover scan failed', description: errorText(err) };
	}
	finally {
		isScanningLeftovers.value = false;
	}
}

function toggleLeftoverSelection(id: string) {
	const next = new Set(selectedLeftoverIds.value);
	if (next.has(id)) {
		next.delete(id);
	}
	else {
		next.add(id);
	}
	selectedLeftoverIds.value = next;
}

function selectAllLeftovers(value: boolean) {
	selectedLeftoverIds.value = value ? new Set(leftovers.value.map(item => item.id)) : new Set();
}

async function confirmDeleteSelectedLeftovers() {
	const toRemove = leftoversToDelete.value;
	if (toRemove.length === 0) {
		confirmDeleteLeftoversOpen.value = false;
		return;
	}
	isDeletingLeftovers.value = true;
	try {
		const report = await deleteLeftoverItems(toRemove);
		confirmDeleteLeftoversOpen.value = false;
		await showDeletionReport(report);
	}
	catch (err: unknown) {
		confirmDeleteLeftoversOpen.value = false;
		banner.value = { color: 'error', title: 'Could not delete leftovers', description: errorText(err) };
	}
	finally {
		isDeletingLeftovers.value = false;
	}
}

async function showDeletionReport(report: DeletionReport) {
	const summary = `Removed ${report.removedCount} item(s), ${formatBytes(report.freedBytes)} moved to the Recycle Bin.`;
	if (report.errors.length === 0 && report.locked.length === 0) {
		banner.value = { color: 'success', title: summary, backupDir: report.backupDir };
	}
	else {
		banner.value = {
			color: 'warning',
			title: `${summary} ${report.errors.length + report.locked.length} item(s) are still there.`,
			description: report.errors.join('\n') || undefined,
			backupDir: report.backupDir
		};
	}
	lockedItems.value = report.locked;
	lockOpen.value = report.locked.length > 0;
	if (selectedApp.value) {
		await triggerLeftoverScan(selectedApp.value);
	}
}

async function openBackups() {
	try {
		await openBackupFolder();
	}
	catch (err: unknown) {
		toast.add({ title: 'Could not open the backup folder', description: errorText(err), color: 'error' });
	}
}

async function executeBatchUninstall() {
	confirmBatchUninstallOpen.value = false;
	const targets = [...selectedBatchApps.value];
	if (targets.length === 0) {
		return;
	}

	banner.value = null;
	isBatchUninstalling.value = true;
	isBatchCancelRequested.value = false;
	batchProgress.value = { current: 0, total: targets.length, currentName: '' };

	const failures: { name: string; error: string }[] = [];
	let successCount = 0;
	let processed = 0;
	let lockedCount = 0;

	await maybeCreateRestorePoint(`Before removing ${targets.length} programs`);

	for (const target of targets) {
		if (isBatchCancelRequested.value) {
			break;
		}
		processed++;
		batchProgress.value = { current: processed, total: targets.length, currentName: target.displayName };

		try {
			const res = await runNativeUninstaller(target, true);
			if (res.stillInstalled) {
				failures.push({ name: target.displayName, error: res.message });
			}
			else {
				// Unattended runs only touch leftovers that are certain: the install folder and the uninstall key.
				const found = await scanAppLeftovers(target);
				const certain = found.filter(item => item.confidence === 'exact-path');
				if (certain.length > 0) {
					const report = await deleteLeftoverItems(certain);
					lockedCount += report.locked.length;
				}
				successCount++;
			}
		}
		catch (err: unknown) {
			failures.push({ name: target.displayName, error: errorText(err) });
		}

		rowSelection.value = Object.fromEntries(Object.entries(rowSelection.value).filter(([id]) => id !== target.id));
	}

	const cancelled = isBatchCancelRequested.value;
	const skipped = targets.length - processed;
	isBatchUninstalling.value = false;
	isBatchCancelRequested.value = false;
	batchProgress.value = null;

	const lockedNote = lockedCount > 0 ? `\n${lockedCount} leftover folder(s) were in use and stayed in place. Open each program to finish cleaning.` : '';
	if (failures.length > 0) {
		banner.value = {
			color: 'error',
			title: `${successCount} removed, ${failures.length} failed${cancelled ? `, ${skipped} skipped` : ''}`,
			description: failures.map(f => `${f.name}: ${f.error}`).join('\n') + lockedNote
		};
	}
	else if (cancelled) {
		banner.value = { color: 'info', title: `Batch stopped. ${successCount} removed, ${skipped} skipped.`, description: lockedNote.trim() || undefined };
	}
	else if (lockedNote) {
		banner.value = { color: 'warning', title: `${successCount} program(s) removed.`, description: lockedNote.trim() };
	}
	else {
		toast.add({ title: 'Batch uninstall finished', description: `${successCount} program(s) removed.`, color: 'success' });
	}

	await refreshApps();
}

async function handleOpenFolder(folderPath: string) {
	try {
		await openInExplorer(folderPath);
	}
	catch (err: unknown) {
		toast.add({ title: 'Could not open Explorer', description: errorText(err), color: 'error' });
	}
}

onMounted(refreshApps);
</script>

<template>
	<UDashboardPanel
		id="programs"
		:default-size="40"
		:min-size="30"
		:max-size="60"
		resizable
	>
		<template #header>
			<UDashboardNavbar title="Installed Programs">
				<template #leading>
					<UDashboardSidebarCollapse />
				</template>
				<template #trailing>
					<UBadge
						:label="String(filteredApps.length)"
						color="neutral"
						variant="subtle"
					/>
				</template>
				<template #right>
					<template v-if="selectedBatchApps.length > 0">
						<UButton
							label="Clear"
							color="neutral"
							variant="ghost"
							:disabled="isBatchUninstalling"
							@click="rowSelection = {}"
						/>
						<UButton
							label="Uninstall"
							icon="i-lucide-trash-2"
							color="error"
							variant="subtle"
							:disabled="isBatchUninstalling || isNativeUninstalling"
							@click="confirmBatchUninstallOpen = true"
						>
							<template #trailing>
								<UKbd>{{ selectedBatchApps.length }}</UKbd>
							</template>
						</UButton>
					</template>
					<UButton
						icon="i-lucide-refresh-cw"
						color="neutral"
						variant="ghost"
						:loading="isLoading"
						:disabled="isBatchUninstalling"
						aria-label="Reload program list"
						@click="refreshApps"
					/>
				</template>
			</UDashboardNavbar>

			<UDashboardToolbar>
				<AppFilters
					v-model:search-query="searchQuery"
					v-model:arch-filter="archFilter"
					v-model:sort-by="sortBy"
					v-model:show-system-components="showSystemComponents"
				/>
			</UDashboardToolbar>
		</template>

		<template #body>
			<AppKpiCards
				v-if="apps.length > 0"
				:total-apps="apps.length"
				:visible-apps="filteredApps.length"
				:total-disk-size-kb="totalDiskSizeKb"
				:x64-apps-count="x64AppsCount"
			/>

			<UCard
				v-if="batchProgress"
				:ui="{ body: 'p-4 sm:p-4 space-y-3' }"
			>
				<div class="flex items-center justify-between gap-3">
					<div class="min-w-0">
						<p class="text-sm font-medium text-highlighted">
							Uninstalling {{ batchProgress.current }} of {{ batchProgress.total }}
						</p>
						<p class="text-xs text-muted truncate">
							{{ batchProgress.currentName }}
						</p>
					</div>
					<UButton
						:label="isBatchCancelRequested ? 'Stopping after this one' : 'Stop'"
						color="neutral"
						variant="outline"
						:disabled="isBatchCancelRequested"
						@click="isBatchCancelRequested = true"
					/>
				</div>
				<UProgress
					:model-value="batchProgress.current - 1"
					:max="batchProgress.total"
					size="sm"
				/>
			</UCard>

			<UAlert
				v-if="banner"
				:color="banner.color"
				variant="subtle"
				:title="banner.title"
				:description="banner.description"
				:ui="{ description: 'whitespace-pre-line' }"
				:actions="banner.backupDir ? [{ label: 'Open registry backup', icon: 'i-lucide-folder-archive', color: 'neutral', variant: 'outline', onClick: openBackups }] : undefined"
				close
				@update:open="banner = null"
			/>

			<UEmpty
				v-if="errorMessage"
				icon="i-lucide-triangle-alert"
				title="Could not read the uninstall registry"
				:description="errorMessage"
				:actions="[{ label: 'Try again', icon: 'i-lucide-refresh-cw', color: 'neutral', variant: 'outline', onClick: refreshApps }]"
				class="flex-1"
			/>

			<div
				v-else-if="isLoading && apps.length === 0"
				class="space-y-3"
				aria-label="Loading installed programs"
			>
				<div
					v-for="n in 8"
					:key="n"
					class="flex items-center gap-3"
				>
					<USkeleton class="size-8 rounded-md" />
					<div class="flex-1 space-y-1.5">
						<USkeleton class="h-4 w-1/2" />
						<USkeleton class="h-3 w-1/3" />
					</div>
					<USkeleton class="h-4 w-14" />
				</div>
			</div>

			<UEmpty
				v-else-if="filteredApps.length === 0"
				:icon="hasActiveFilter ? 'i-lucide-search-x' : 'i-lucide-package-open'"
				:title="hasActiveFilter ? 'No programs match these filters' : 'No programs found'"
				:description="hasActiveFilter
					? 'Try a shorter search or switch the architecture filter back to All.'
					: 'The uninstall registry returned no entries. Turn on system components or reload the list.'"
				:actions="hasActiveFilter
					? [{ label: 'Clear filters', color: 'neutral', variant: 'outline', onClick: resetFilters }]
					: [{ label: 'Reload', icon: 'i-lucide-refresh-cw', color: 'neutral', variant: 'outline', onClick: refreshApps }]"
				class="flex-1"
			/>

			<UTable
				v-else
				v-model:row-selection="rowSelection"
				:data="filteredApps"
				:columns="columns"
				:meta="tableMeta"
				:get-row-id="(row: InstalledApp) => row.id"
				:loading="isLoading"
				sticky
				class="shrink-0"
				:ui="{
					base: 'table-fixed border-separate border-spacing-0',
					thead: '[&>tr]:bg-elevated/50 [&>tr]:after:content-none',
					tbody: '[&>tr]:last:[&>td]:border-b-0',
					th: 'py-2 first:rounded-l-lg last:rounded-r-lg border-y border-default first:border-l last:border-r',
					td: 'border-b border-default cursor-pointer',
					separator: 'h-0'
				}"
				@select="(_e: Event, row: TableRow<InstalledApp>) => selectProgram(row.original)"
			/>
		</template>
	</UDashboardPanel>

	<template v-if="!isMobile">
		<AppInspector
			v-if="selectedApp"
			:app="selectedApp"
			:icon-src="iconFor(selectedApp)"
			:is-native-uninstalling="isNativeUninstalling"
			:has-completed-native-uninstall="hasCompletedNativeUninstall"
			:is-scanning-leftovers="isScanningLeftovers"
			:has-scanned-current-app="hasScannedCurrentApp"
			:leftovers="leftovers"
			:selected-leftover-ids="selectedLeftoverIds"
			:is-deleting-leftovers="isDeletingLeftovers"
			:is-batch-running="isBatchUninstalling"
			@close="selectedApp = null"
			@uninstall="(quiet) => selectedApp && requestSingleUninstall(selectedApp, quiet)"
			@rescan="selectedApp && triggerLeftoverScan(selectedApp)"
			@open-folder="handleOpenFolder"
			@toggle-leftover="toggleLeftoverSelection"
			@select-all-leftovers="selectAllLeftovers"
			@request-delete-leftovers="confirmDeleteLeftoversOpen = true"
		/>
		<div
			v-else
			class="flex flex-1 items-center justify-center p-8"
		>
			<UEmpty
				icon="i-lucide-mouse-pointer-click"
				variant="naked"
				title="Pick a program"
				description="Its install folder, registry key and uninstall options show up here."
			/>
		</div>
	</template>

	<ClientOnly>
		<USlideover
			v-if="isMobile"
			v-model:open="inspectorOpen"
			:title="selectedApp?.displayName"
		>
			<template #content>
				<AppInspector
					v-if="selectedApp"
					:app="selectedApp"
					:icon-src="iconFor(selectedApp)"
					:is-native-uninstalling="isNativeUninstalling"
					:has-completed-native-uninstall="hasCompletedNativeUninstall"
					:is-scanning-leftovers="isScanningLeftovers"
					:has-scanned-current-app="hasScannedCurrentApp"
					:leftovers="leftovers"
					:selected-leftover-ids="selectedLeftoverIds"
					:is-deleting-leftovers="isDeletingLeftovers"
					:is-batch-running="isBatchUninstalling"
					@close="selectedApp = null"
					@uninstall="(quiet) => selectedApp && requestSingleUninstall(selectedApp, quiet)"
					@rescan="selectedApp && triggerLeftoverScan(selectedApp)"
					@open-folder="handleOpenFolder"
					@toggle-leftover="toggleLeftoverSelection"
					@select-all-leftovers="selectAllLeftovers"
					@request-delete-leftovers="confirmDeleteLeftoversOpen = true"
				/>
			</template>
		</USlideover>
	</ClientOnly>

	<UModal
		v-model:open="confirmDeleteLeftoversOpen"
		title="Delete leftovers?"
		:description="`${leftoversToDelete.length} item(s) go to the Recycle Bin. Registry keys are saved as .reg backups first.`"
		:dismissible="!isDeletingLeftovers"
	>
		<template #body>
			<div class="space-y-3">
				<UAlert
					v-if="riskyLeftoversToDelete.length > 0"
					color="warning"
					variant="subtle"
					icon="i-lucide-triangle-alert"
					:title="`${riskyLeftoversToDelete.length} item(s) need a second look`"
					description="They were matched by name, or the registered folder does not look like the program's. Uncheck any you do not recognise."
				/>
				<ul class="max-h-60 overflow-y-auto divide-y divide-default rounded-md ring ring-default">
					<li
						v-for="item in leftoversToDelete"
						:key="item.id"
						class="flex items-start gap-2 px-3 py-2"
					>
						<UBadge
							:label="item.kind"
							:color="isRiskyLeftover(item) ? 'warning' : 'neutral'"
							variant="subtle"
							size="sm"
							class="shrink-0 capitalize"
						/>
						<span class="font-mono text-xs text-highlighted break-all">{{ item.path }}</span>
					</li>
				</ul>
			</div>
		</template>
		<template #footer>
			<div class="flex w-full justify-end gap-2">
				<UButton
					label="No, Cancel"
					color="neutral"
					variant="outline"
					:disabled="isDeletingLeftovers"
					@click="confirmDeleteLeftoversOpen = false"
				/>
				<UButton
					:label="`Yes, Delete ${leftoversToDelete.length} item(s)`"
					color="error"
					variant="solid"
					:loading="isDeletingLeftovers"
					@click="confirmDeleteSelectedLeftovers"
				/>
			</div>
		</template>
	</UModal>

	<UModal
		v-model:open="confirmBatchUninstallOpen"
		title="Uninstall selected programs?"
		:description="`${selectedBatchApps.length} program(s), ${formatKb(selectedBatchSizeKb)} in total. They run one after another.`"
	>
		<template #body>
			<div class="space-y-3">
				<p class="text-sm text-muted">
					Each uninstaller runs in quiet mode when it supports it. When one finishes and the program is really gone, its install folder and uninstall entry go to the Recycle Bin. Anything matched only by name is left for you to review.
				</p>
				<RestorePointOption v-model="makeRestorePoint" />
				<ul class="max-h-60 overflow-y-auto divide-y divide-default rounded-md ring ring-default">
					<li
						v-for="app in selectedBatchApps"
						:key="app.id"
						class="flex items-center justify-between gap-3 px-3 py-2 text-sm"
					>
						<span class="truncate text-highlighted">{{ app.displayName }}</span>
						<span class="shrink-0 text-muted tabular-nums">{{ formatKb(app.estimatedSizeKb) }}</span>
					</li>
				</ul>
			</div>
		</template>
		<template #footer>
			<div class="flex w-full justify-end gap-2">
				<UButton
					label="No, Cancel"
					color="neutral"
					variant="outline"
					@click="confirmBatchUninstallOpen = false"
				/>
				<UButton
					:label="`Yes, Uninstall ${selectedBatchApps.length}`"
					color="error"
					variant="solid"
					@click="executeBatchUninstall"
				/>
			</div>
		</template>
	</UModal>

	<!-- Confirmation Modal: Single Application Uninstall -->
	<UModal
		v-model:open="confirmSingleUninstallOpen"
		:title="`Uninstall ${appToUninstall?.app.displayName}?`"
		:description="`Are you sure you want to uninstall this application? ${appToUninstall?.preferQuiet ? 'Quiet mode will be executed.' : 'The official setup uninstaller wizard will be launched.'}`"
		:dismissible="!isNativeUninstalling"
	>
		<template #body>
			<div
				v-if="appToUninstall"
				class="space-y-3 text-xs"
			>
				<div class="p-3 rounded-lg bg-elevated border border-default space-y-1.5">
					<div class="flex items-center justify-between gap-2">
						<span class="font-semibold text-highlighted text-sm truncate">
							{{ appToUninstall.app.displayName }}
						</span>
						<UBadge
							:label="appToUninstall.app.architecture"
							color="neutral"
							variant="subtle"
							size="sm"
						/>
					</div>
					<p class="text-muted">
						{{ appToUninstall.app.publisher || 'Unknown publisher' }} • {{ formatKb(appToUninstall.app.estimatedSizeKb) }}
					</p>
					<p
						v-if="appToUninstall.app.installLocation"
						class="font-mono text-muted text-xs truncate"
					>
						{{ appToUninstall.app.installLocation }}
					</p>
				</div>
				<p class="text-muted">
					When the uninstaller finishes, Tuntas checks the program is really gone and then looks for folders, shortcuts and registry keys it left behind. Nothing is deleted until you confirm.
				</p>
				<RestorePointOption v-model="makeRestorePoint" />
			</div>
		</template>
		<template #footer>
			<div class="flex w-full justify-end gap-2">
				<UButton
					label="No, Cancel"
					color="neutral"
					variant="outline"
					:disabled="isNativeUninstalling"
					@click="confirmSingleUninstallOpen = false"
				/>
				<UButton
					label="Yes, Uninstall"
					color="error"
					variant="solid"
					:loading="isNativeUninstalling"
					@click="executeSingleUninstall"
				/>
			</div>
		</template>
	</UModal>

	<LockResolver
		v-model:open="lockOpen"
		:locked="lockedItems"
		:items="leftovers"
		@resolved="showDeletionReport"
		@scheduled="lockedItems = []"
	/>
</template>
