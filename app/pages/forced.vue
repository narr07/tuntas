<script setup lang="ts">
import type { TableColumn, TableRow } from '@nuxt/ui';
import type { DeletionReport, InstalledApp, LeftoverItem, LockedItem } from '~/composables/useUninstallerBackend';
import {
	confidenceLabel,
	errorText,
	formatBytes,
	formatKb,
	isPreselectedLeftover,
	isRiskyLeftover,
	useAppIcons,
	useUninstallerBackend
} from '~/composables/useUninstallerBackend';
import { h, resolveComponent } from 'vue';

const UCheckbox = resolveComponent('UCheckbox');
const UBadge = resolveComponent('UBadge');
const UIcon = resolveComponent('UIcon');
const UButton = resolveComponent('UButton');
const UAvatar = resolveComponent('UAvatar');
const toast = useToast();
const route = useRoute();

const {
	forcedUninstallScan,
	deleteLeftoverItems,
	listInstalledApps,
	openBackupFolder
} = useUninstallerBackend();
const { load: loadIcons, iconFor } = useAppIcons();

// Form Inputs
const targetName = ref((route.query.name as string) || '');
const publisher = ref((route.query.publisher as string) || '');
const targetPath = ref((route.query.path as string) || '');

// Installed Programs List for Quick Selection
const installedApps = ref<InstalledApp[]>([]);
const isLoadingApps = ref(false);
const appSearchQuery = ref('');

// Forced Scan State
const isScanning = ref(false);
const isDeleting = ref(false);
const errorMessage = ref<string | null>(null);
const hasScanned = ref(false);
const results = ref<LeftoverItem[]>([]);
const rowSelection = ref<Record<string, boolean>>({});
const report = ref<DeletionReport | null>(null);
const confirmDeleteOpen = ref(false);
const lockOpen = ref(false);
const lockedItems = ref<LockedItem[]>([]);

const selectedItems = computed(() => results.value.filter(i => rowSelection.value[i.id]));

const selectedBytes = computed(() => {
	let sum = 0;
	for (const item of selectedItems.value) {
		sum += item.sizeBytes;
	}
	return sum;
});

const riskySelectedCount = computed(() => {
	return selectedItems.value.filter(isRiskyLeftover).length;
});

// Filtered installed apps for the picker table
const filteredInstalledApps = computed(() => {
	const q = appSearchQuery.value.trim().toLowerCase();
	if (!q) {
		return installedApps.value;
	}
	return installedApps.value.filter(app =>
		app.displayName.toLowerCase().includes(q)
		|| app.publisher.toLowerCase().includes(q)
		|| app.installLocation.toLowerCase().includes(q)
	);
});

async function loadInstalledApps() {
	isLoadingApps.value = true;
	try {
		const list = await listInstalledApps();
		installedApps.value = list;
		loadIcons(list);
	}
	catch (err: unknown) {
		console.error('Failed to preload installed apps for forced picker:', err);
	}
	finally {
		isLoadingApps.value = false;
	}
}

function selectAppToForce(app: InstalledApp) {
	targetName.value = app.displayName;
	publisher.value = app.publisher || '';
	targetPath.value = app.installLocation || '';
	runForcedScan();
}

// Columns for Installed Apps Picker Table
const appColumns: TableColumn<InstalledApp>[] = [
	{
		accessorKey: 'displayName',
		header: 'Program Name',
		cell: ({ row }) => {
			const app = row.original;
			return h('div', { class: 'flex items-center gap-3 py-1 min-w-0' }, [
				h(UAvatar, {
					src: iconFor(app),
					alt: app.displayName,
					size: 'md',
					class: 'rounded-md bg-elevated ring ring-default shrink-0'
				}),
				h('div', { class: 'min-w-0 flex-1' }, [
					h('div', { class: 'flex items-center gap-1.5' }, [
						h('p', { class: 'font-medium text-highlighted truncate' }, app.displayName),
						h(UBadge, { label: app.architecture, color: 'neutral', variant: 'subtle', size: 'sm' })
					]),
					h('p', { class: 'font-mono text-xs text-muted truncate mt-0.5' }, app.publisher || 'Unknown publisher')
				])
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'installLocation',
		header: 'Installation Folder',
		cell: ({ row }) => row.original.installLocation || 'Not recorded in registry',
		meta: { class: { th: 'hidden md:table-cell w-72', td: 'hidden md:table-cell font-mono text-xs text-muted truncate max-w-xs' } }
	},
	{
		accessorKey: 'estimatedSizeKb',
		header: 'Size',
		cell: ({ row }) => formatKb(row.original.estimatedSizeKb),
		meta: { class: { th: 'text-right w-28', td: 'text-right tabular-nums text-muted text-xs whitespace-nowrap w-28' } }
	},
	{
		id: 'actions',
		header: 'Action',
		meta: { class: { th: 'text-right w-32', td: 'text-right whitespace-nowrap w-32' } },
		cell: ({ row }) => {
			const app = row.original;
			return h(UButton, {
				label: 'Force Scan',
				icon: 'i-lucide-radar',
				color: 'primary',
				variant: 'subtle',
				size: 'xs',
				onClick: (e: Event) => {
					e.stopPropagation();
					selectAppToForce(app);
				}
			});
		}
	}
];

// Columns for Remnants Result Table
const remnantColumns: TableColumn<LeftoverItem>[] = [
	{
		id: 'select',
		header: ({ table }) => h(UCheckbox, {
			'modelValue': table.getIsSomePageRowsSelected() ? 'indeterminate' : table.getIsAllPageRowsSelected(),
			'onUpdate:modelValue': (v: boolean | 'indeterminate') => table.toggleAllPageRowsSelected(Boolean(v)),
			'aria-label': 'Select all remnants'
		}),
		cell: ({ row }) => h(UCheckbox, {
			'modelValue': row.getIsSelected(),
			'onUpdate:modelValue': (v: boolean | 'indeterminate') => row.toggleSelected(Boolean(v)),
			'onClick': (e: Event) => e.stopPropagation(),
			'aria-label': `Select ${row.original.path}`
		}),
		meta: { class: { th: 'w-10', td: 'w-10' } }
	},
	{
		accessorKey: 'kind',
		header: 'Type',
		cell: ({ row }) => {
			const item = row.original;
			const isRisky = isRiskyLeftover(item);
			return h('div', { class: 'flex items-center gap-1.5' }, [
				h(UIcon, {
					name: item.kind === 'registry' ? 'i-lucide-database' : (item.kind === 'file' ? 'i-lucide-file' : 'i-lucide-folder'),
					class: 'size-4 shrink-0 text-muted'
				}),
				h(UBadge, {
					label: item.kind,
					color: isRisky ? 'warning' : 'neutral',
					variant: 'subtle',
					size: 'sm',
					class: 'capitalize'
				})
			]);
		},
		meta: { class: { th: 'w-28', td: 'w-28 whitespace-nowrap' } }
	},
	{
		accessorKey: 'path',
		header: 'Target Path / Registry Key',
		cell: ({ row }) => {
			const item = row.original;
			return h('div', { class: 'min-w-0 py-1' }, [
				h('p', { class: 'font-mono text-xs text-highlighted break-all' }, item.path),
				h('p', { class: 'text-xs text-muted mt-0.5' }, item.reason)
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'confidence',
		header: 'Match',
		cell: ({ row }) => {
			const item = row.original;
			return h(UBadge, {
				label: confidenceLabel[item.confidence],
				color: isRiskyLeftover(item) ? 'warning' : 'neutral',
				variant: 'outline',
				size: 'sm'
			});
		},
		meta: { class: { th: 'w-32', td: 'w-32 whitespace-nowrap' } }
	},
	{
		accessorKey: 'sizeBytes',
		header: 'Size',
		cell: ({ row }) => {
			const item = row.original;
			return item.kind === 'registry' ? '-' : formatBytes(item.sizeBytes);
		},
		meta: { class: { th: 'text-right w-24', td: 'text-right tabular-nums text-muted whitespace-nowrap w-24' } }
	}
];

async function runForcedScan() {
	if (!targetName.value.trim() && !targetPath.value.trim()) {
		errorMessage.value = 'Enter a program name or folder path to begin scanning.';
		return;
	}
	errorMessage.value = null;
	report.value = null;
	isScanning.value = true;
	hasScanned.value = false;
	rowSelection.value = {};
	try {
		const items = await forcedUninstallScan(
			targetName.value.trim(),
			publisher.value.trim(),
			targetPath.value.trim()
		);
		results.value = items;
		// Tick only what is clearly the program's; matches by name stay for the user to review.
		const sel: Record<string, boolean> = {};
		for (const item of items.filter(isPreselectedLeftover)) {
			sel[item.id] = true;
		}
		rowSelection.value = sel;
		hasScanned.value = true;
	}
	catch (err: unknown) {
		errorMessage.value = errorText(err);
	}
	finally {
		isScanning.value = false;
	}
}

function resetForm() {
	targetName.value = '';
	publisher.value = '';
	targetPath.value = '';
	results.value = [];
	rowSelection.value = {};
	hasScanned.value = false;
	errorMessage.value = null;
	report.value = null;
}

async function confirmPurge() {
	const chosen = selectedItems.value;
	if (chosen.length === 0) {
		confirmDeleteOpen.value = false;
		return;
	}
	isDeleting.value = true;
	errorMessage.value = null;
	try {
		const res = await deleteLeftoverItems(chosen);
		confirmDeleteOpen.value = false;
		await handleDeletionReport(res);
		await loadInstalledApps();
	}
	catch (err: unknown) {
		confirmDeleteOpen.value = false;
		errorMessage.value = errorText(err);
		toast.add({
			title: 'Purge failed',
			description: errorMessage.value,
			color: 'error'
		});
	}
	finally {
		isDeleting.value = false;
	}
}

async function handleDeletionReport(res: DeletionReport) {
	report.value = res;
	lockedItems.value = res.locked;
	lockOpen.value = res.locked.length > 0;
	toast.add({
		title: 'Clean-up finished',
		description: `Removed ${res.removedCount} item(s), ${formatBytes(res.freedBytes)} moved to the Recycle Bin.`,
		color: res.errors.length === 0 && res.locked.length === 0 ? 'success' : 'warning'
	});
	await runForcedScan();
}

async function openBackups() {
	try {
		await openBackupFolder();
	}
	catch (err: unknown) {
		toast.add({ title: 'Could not open the backup folder', description: errorText(err), color: 'error' });
	}
}

onMounted(async () => {
	await loadInstalledApps();
	if (targetName.value || targetPath.value) {
		runForcedScan();
	}
});
</script>

<template>
	<UDashboardPanel id="forced-uninstall">
		<template #header>
			<UDashboardNavbar title="Forced Uninstall">
				<template #leading>
					<UDashboardSidebarCollapse />
				</template>
				<template #trailing>
					<UBadge
						v-if="hasScanned"
						:label="`${results.length} remnants found`"
						color="neutral"
						variant="subtle"
					/>
					<UBadge
						v-else-if="installedApps.length > 0"
						:label="`${installedApps.length} installed apps`"
						color="neutral"
						variant="subtle"
					/>
				</template>
				<template #right>
					<UButton
						v-if="hasScanned"
						label="Back to Program List"
						icon="i-lucide-arrow-left"
						color="neutral"
						variant="outline"
						size="sm"
						@click="hasScanned = false; results = [];"
					/>
					<UButton
						icon="i-lucide-refresh-cw"
						color="neutral"
						variant="ghost"
						:loading="isLoadingApps"
						aria-label="Reload installed programs"
						@click="loadInstalledApps"
					/>
				</template>
			</UDashboardNavbar>
		</template>

		<template #body>
			<div class="space-y-4">
				<!-- Search / Target Input Card -->
				<UCard :ui="{ body: 'p-4 sm:p-5 space-y-4' }">
					<div class="flex flex-wrap items-center justify-between gap-2">
						<div>
							<h2 class="text-sm font-semibold text-highlighted">
								Target Application Details
							</h2>
							<p class="text-xs text-muted mt-0.5">
								Scan AppData, ProgramData, and Windows Registry hives for broken installations or leftover directories.
							</p>
						</div>

						<span class="text-xs font-mono text-muted">
							Windows, Program Files roots and your user folders are always protected.
						</span>
					</div>

					<form
						class="space-y-4"
						@submit.prevent="runForcedScan"
					>
						<div class="grid grid-cols-1 md:grid-cols-3 gap-3">
							<UFormField
								label="Program Name"
								description="Name or token of the program"
							>
								<UInput
									v-model="targetName"
									placeholder="e.g. Zoom, Slack, Postman"
									icon="i-lucide-type"
									class="w-full"
								/>
							</UFormField>

							<UFormField
								label="Publisher"
								description="Optional vendor name"
							>
								<UInput
									v-model="publisher"
									placeholder="e.g. Microsoft, Adobe"
									icon="i-lucide-building"
									class="w-full"
								/>
							</UFormField>

							<UFormField
								label="Installation Folder"
								description="Optional full path to directory"
							>
								<UInput
									v-model="targetPath"
									placeholder="e.g. C:\Program Files\Target"
									icon="i-lucide-folder"
									class="w-full font-mono text-xs"
								/>
							</UFormField>
						</div>

						<div class="flex flex-wrap items-center justify-between gap-3 pt-2 border-t border-default">
							<p class="text-xs text-muted">
								You can fill the form above, or click any application from the list below to scan automatically.
							</p>

							<div class="flex items-center gap-2">
								<UButton
									label="Reset"
									color="neutral"
									variant="outline"
									size="sm"
									:disabled="isScanning || (!targetName && !publisher && !targetPath)"
									@click="resetForm"
								/>
								<UButton
									type="submit"
									label="Scan for Remnants"
									icon="i-lucide-radar"
									color="primary"
									variant="solid"
									size="sm"
									:loading="isScanning"
									:disabled="!targetName.trim() && !targetPath.trim()"
								/>
							</div>
						</div>
					</form>
				</UCard>

				<!-- Deletion Report Alert -->
				<UAlert
					v-if="report"
					:color="report.errors.length > 0 || report.locked.length > 0 ? 'warning' : 'success'"
					variant="subtle"
					icon="i-lucide-circle-check"
					:title="`Removed ${report.removedCount} item(s), ${formatBytes(report.freedBytes)} moved to the Recycle Bin`"
					:description="report.errors.length > 0 ? report.errors.join('\n') : undefined"
					:ui="{ description: 'whitespace-pre-line' }"
					:actions="report.backupDir ? [{ label: 'Open registry backup', icon: 'i-lucide-folder-archive', color: 'neutral', variant: 'outline', onClick: openBackups }] : undefined"
					close
					@update:open="report = null"
				/>

				<!-- Error Alert -->
				<UAlert
					v-if="errorMessage"
					color="error"
					variant="subtle"
					icon="i-lucide-triangle-alert"
					title="Forced scan error"
					:description="errorMessage"
					close
					@update:open="errorMessage = null"
				/>

				<!-- Scanning State Skeleton -->
				<div
					v-if="isScanning"
					class="space-y-3 py-6"
				>
					<div class="flex items-center gap-2 text-xs text-muted">
						<UIcon
							name="i-lucide-radar"
							class="size-4 animate-spin text-primary"
						/>
						<span>Scanning Windows Registry, AppData, and ProgramData for matching remnants...</span>
					</div>
					<div
						v-for="n in 6"
						:key="n"
						class="flex items-center gap-3"
					>
						<USkeleton class="size-6 rounded" />
						<USkeleton class="h-4 w-24 rounded" />
						<div class="flex-1 space-y-1">
							<USkeleton class="h-4 w-3/4 rounded" />
							<USkeleton class="h-3 w-1/2 rounded" />
						</div>
						<USkeleton class="h-4 w-16 rounded" />
					</div>
				</div>

				<!-- STATE 1: RESULTS VIEW (Shown when scan has run) -->
				<div
					v-else-if="hasScanned"
					class="space-y-3"
				>
					<UEmpty
						v-if="results.length === 0"
						icon="i-lucide-check-circle"
						title="No residual remnants detected"
						description="No orphaned folders, files, or registry keys matched the provided name or directory token."
						variant="naked"
						class="py-12"
						:actions="[{
							label: 'Pick another application',
							icon: 'i-lucide-arrow-left',
							color: 'neutral',
							variant: 'outline',
							onClick: () => { hasScanned = false; results = []; }
						}]"
					/>

					<div
						v-else
						class="space-y-3"
					>
						<!-- Bulk Selection Action Bar -->
						<div class="flex flex-wrap items-center justify-between gap-3 p-3 rounded-lg bg-elevated/40 border border-default">
							<div class="flex items-center gap-2 text-xs">
								<span class="font-semibold text-highlighted">
									{{ selectedItems.length }} of {{ results.length }} items selected
								</span>
								<span class="text-muted">
									({{ formatBytes(selectedBytes) }})
								</span>
							</div>

							<div class="flex items-center gap-2">
								<UButton
									label="Select all"
									color="neutral"
									variant="ghost"
									size="xs"
									@click="() => {
										const sel: Record<string, boolean> = {};
										for (const i of results) sel[i.id] = true;
										rowSelection = sel;
									}"
								/>
								<UButton
									label="Clear"
									color="neutral"
									variant="ghost"
									size="xs"
									@click="rowSelection = {}"
								/>
								<UButton
									label="Purge selected"
									icon="i-lucide-trash-2"
									color="error"
									variant="solid"
									size="sm"
									:disabled="selectedItems.length === 0 || isDeleting"
									:loading="isDeleting"
									@click="confirmDeleteOpen = true"
								/>
							</div>
						</div>

						<!-- Remnants Table -->
						<UTable
							v-model:row-selection="rowSelection"
							:data="results"
							:columns="remnantColumns"
							:get-row-id="(row: LeftoverItem) => row.id"
							sticky
							class="shrink-0"
							:ui="{
								base: 'table-fixed border-separate border-spacing-0',
								thead: '[&>tr]:bg-elevated/50 [&>tr]:after:content-none',
								tbody: '[&>tr]:last:[&>td]:border-b-0',
								th: 'py-2 first:rounded-l-lg last:rounded-r-lg border-y border-default first:border-l last:border-r',
								td: 'border-b border-default'
							}"
						/>
					</div>
				</div>

				<!-- STATE 2: INSTALLED APPS PICKER (Shown before scan so user can easily choose any app!) -->
				<div
					v-else
					class="space-y-3 pt-1"
				>
					<div class="flex flex-wrap items-center justify-between gap-3">
						<div>
							<h3 class="text-xs font-semibold uppercase tracking-wider text-muted">
								Or Select an Installed Program to Force Uninstall
							</h3>
							<p class="text-xs text-muted mt-0.5">
								Click any application to automatically populate its name, publisher, and path, then perform a deep leftover scan.
							</p>
						</div>

						<UInput
							v-model="appSearchQuery"
							icon="i-lucide-search"
							placeholder="Search installed programs..."
							class="w-full sm:w-64"
							size="xs"
							aria-label="Filter programs to force uninstall"
						/>
					</div>

					<div
						v-if="isLoadingApps && installedApps.length === 0"
						class="space-y-2 p-4"
					>
						<div
							v-for="n in 6"
							:key="n"
							class="flex items-center gap-3"
						>
							<USkeleton class="size-8 rounded-md" />
							<div class="flex-1 space-y-1">
								<USkeleton class="h-4 w-1/3" />
								<USkeleton class="h-3 w-1/2" />
							</div>
							<USkeleton class="h-4 w-16" />
							<USkeleton class="h-8 w-24 rounded" />
						</div>
					</div>

					<UEmpty
						v-else-if="filteredInstalledApps.length === 0"
						icon="i-lucide-package-open"
						:title="appSearchQuery ? 'No installed programs match your search' : 'No installed programs detected'"
						:description="appSearchQuery ? 'Try a different search term.' : 'Registry uninstall hives returned zero apps.'"
						variant="naked"
						class="py-8"
					/>

					<UTable
						v-else
						:data="filteredInstalledApps"
						:columns="appColumns"
						:loading="isLoadingApps"
						sticky
						class="shrink-0"
						:ui="{
							base: 'table-fixed border-separate border-spacing-0',
							thead: '[&>tr]:bg-elevated/50 [&>tr]:after:content-none',
							tbody: '[&>tr]:last:[&>td]:border-b-0',
							th: 'py-2 first:rounded-l-lg last:rounded-r-lg border-y border-default first:border-l last:border-r',
							td: 'border-b border-default cursor-pointer hover:bg-elevated/40'
						}"
						@select="(_e: Event, row: TableRow<InstalledApp>) => selectAppToForce(row.original)"
					/>
				</div>
			</div>
		</template>
	</UDashboardPanel>

	<!-- Confirmation Modal -->
	<UModal
		v-model:open="confirmDeleteOpen"
		title="Delete these leftovers?"
		:description="`${selectedItems.length} remnant item(s) will go to the Recycle Bin (${formatBytes(selectedBytes)}). Registry keys are saved as .reg backups first.`"
		:dismissible="!isDeleting"
	>
		<template #body>
			<div class="space-y-3">
				<UAlert
					v-if="riskySelectedCount > 0"
					color="warning"
					variant="subtle"
					icon="i-lucide-triangle-alert"
					:title="`${riskySelectedCount} item(s) need a second look`"
					description="They were matched by name, or the folder name does not look like the program's. Make sure they do not belong to something you still use."
				/>

				<ul class="max-h-60 overflow-y-auto divide-y divide-default rounded-md border border-default">
					<li
						v-for="item in selectedItems"
						:key="item.id"
						class="p-2.5 text-xs flex items-start gap-2"
					>
						<UBadge
							:label="item.kind"
							:color="isRiskyLeftover(item) ? 'warning' : 'neutral'"
							variant="subtle"
							size="sm"
							class="shrink-0 uppercase"
						/>
						<span class="font-mono text-highlighted break-all">{{ item.path }}</span>
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
					:disabled="isDeleting"
					@click="confirmDeleteOpen = false"
				/>
				<UButton
					:label="`Yes, Purge ${selectedItems.length} Item(s)`"
					color="error"
					variant="solid"
					:loading="isDeleting"
					@click="confirmPurge"
				/>
			</div>
		</template>
	</UModal>

	<LockResolver
		v-model:open="lockOpen"
		:locked="lockedItems"
		:items="results"
		@resolved="handleDeletionReport"
		@scheduled="lockedItems = []"
	/>
</template>
