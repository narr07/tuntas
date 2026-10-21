<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui';
import type { ActiveProcessItem } from '~/composables/useUninstallerBackend';
import { errorText, formatBytes, useUninstallerBackend } from '~/composables/useUninstallerBackend';
import { h, resolveComponent } from 'vue';

const UButton = resolveComponent('UButton');
const UBadge = resolveComponent('UBadge');
const toast = useToast();
const router = useRouter();

const { listActiveProcesses, terminateProcessByPid, openInExplorer } = useUninstallerBackend();

const processes = ref<ActiveProcessItem[]>([]);
const isLoading = ref(false);
const errorMessage = ref<string | null>(null);
const searchQuery = ref('');
const onlyMatchedInstalled = ref(false);

const confirmKillModalOpen = ref(false);
const processToKill = ref<ActiveProcessItem | null>(null);
const isTerminating = ref(false);

const filteredProcesses = computed(() => {
	const q = searchQuery.value.trim().toLowerCase();
	return processes.value.filter((proc) => {
		if (onlyMatchedInstalled.value && !proc.matchedAppName) {
			return false;
		}
		if (!q) {
			return true;
		}
		return (
			proc.name.toLowerCase().includes(q)
			|| proc.exePath.toLowerCase().includes(q)
			|| String(proc.pid).includes(q)
			|| (proc.matchedAppName && proc.matchedAppName.toLowerCase().includes(q))
		);
	});
});

const columns: TableColumn<ActiveProcessItem>[] = [
	{
		accessorKey: 'pid',
		header: 'PID',
		cell: ({ row }) => `#${row.original.pid}`,
		meta: { class: { th: 'w-20 font-mono', td: 'w-20 font-mono text-xs text-muted whitespace-nowrap' } }
	},
	{
		accessorKey: 'name',
		header: 'Process / Executable',
		cell: ({ row }) => {
			const proc = row.original;
			return h('div', { class: 'min-w-0 py-1' }, [
				h('div', { class: 'flex items-center gap-2' }, [
					h('p', { class: 'font-medium text-highlighted truncate' }, proc.name),
					proc.matchedAppName
						? h(UBadge, { label: proc.matchedAppName, color: 'primary', variant: 'subtle', size: 'sm', class: 'shrink-0' })
						: null
				]),
				h('p', { class: 'font-mono text-xs text-muted truncate mt-0.5' }, proc.exePath || 'System Process')
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'workingSetBytes',
		header: 'RAM Usage',
		cell: ({ row }) => formatBytes(row.original.workingSetBytes),
		meta: { class: { th: 'text-right w-28', td: 'text-right tabular-nums text-muted text-xs whitespace-nowrap w-28' } }
	},
	{
		id: 'actions',
		header: 'Actions',
		meta: { class: { th: 'text-right w-44', td: 'text-right whitespace-nowrap w-44' } },
		cell: ({ row }) => {
			const proc = row.original;
			return h('div', { class: 'flex items-center justify-end gap-1.5' }, [
				proc.exePath
					? h(UButton, {
							'icon': 'i-lucide-folder-open',
							'color': 'neutral',
							'variant': 'ghost',
							'size': 'xs',
							'aria-label': 'Locate in Explorer',
							'onClick': () => handleOpenPath(proc.exePath)
						})
					: null,
				h(UButton, {
					label: 'Purge',
					icon: 'i-lucide-shield-alert',
					color: 'neutral',
					variant: 'outline',
					size: 'xs',
					onClick: () => jumpToForcedScan(proc)
				}),
				h(UButton, {
					label: 'Close',
					icon: 'i-lucide-x-circle',
					color: 'error',
					variant: 'subtle',
					size: 'xs',
					onClick: () => requestKill(proc)
				})
			]);
		}
	}
];

async function loadProcesses() {
	isLoading.value = true;
	errorMessage.value = null;
	try {
		processes.value = await listActiveProcesses();
	}
	catch (err: unknown) {
		errorMessage.value = errorText(err);
	}
	finally {
		isLoading.value = false;
	}
}

function requestKill(proc: ActiveProcessItem) {
	processToKill.value = proc;
	confirmKillModalOpen.value = true;
}

async function confirmKillProcess() {
	if (!processToKill.value) return;
	const proc = processToKill.value;
	isTerminating.value = true;
	try {
		const msg = await terminateProcessByPid(proc.pid);
		confirmKillModalOpen.value = false;
		toast.add({
			title: 'Process terminated',
			description: msg || `Terminated PID ${proc.pid} (${proc.name}).`,
			color: 'success'
		});
		await loadProcesses();
	}
	catch (err: unknown) {
		toast.add({
			title: 'Termination failed',
			description: errorText(err),
			color: 'error'
		});
	}
	finally {
		isTerminating.value = false;
		processToKill.value = null;
	}
}

async function handleOpenPath(exePath: string) {
	try {
		await openInExplorer(exePath);
	}
	catch (err: unknown) {
		toast.add({
			title: 'Could not open folder',
			description: errorText(err),
			color: 'error'
		});
	}
}

function jumpToForcedScan(proc: ActiveProcessItem) {
	const parentDir = proc.exePath ? proc.exePath.replace(/\\[^\\]+$/, '') : '';
	router.push({
		path: '/forced',
		query: {
			name: proc.matchedAppName || proc.name,
			path: parentDir
		}
	});
}

onMounted(loadProcesses);
</script>

<template>
	<UDashboardPanel id="hunter">
		<template #header>
			<UDashboardNavbar title="Hunter Mode">
				<template #leading>
					<UDashboardSidebarCollapse />
				</template>
				<template #trailing>
					<UBadge
						:label="`${filteredProcesses.length} / ${processes.length} PIDs`"
						color="neutral"
						variant="subtle"
					/>
				</template>
				<template #right>
					<UButton
						icon="i-lucide-refresh-cw"
						color="neutral"
						variant="ghost"
						:loading="isLoading"
						aria-label="Rescan running processes"
						@click="loadProcesses"
					/>
				</template>
			</UDashboardNavbar>

			<UDashboardToolbar>
				<div class="flex w-full flex-wrap items-center justify-between gap-3 py-1">
					<UInput
						v-model="searchQuery"
						icon="i-lucide-search"
						placeholder="Search PID, executable name, path..."
						class="w-full sm:w-80"
						aria-label="Filter active processes"
					/>

					<USwitch
						v-model="onlyMatchedInstalled"
						label="Only show processes linked to installed software"
						size="sm"
					/>
				</div>
			</UDashboardToolbar>
		</template>

		<template #body>
			<UEmpty
				v-if="errorMessage"
				icon="i-lucide-triangle-alert"
				title="Could not query active Windows processes"
				:description="errorMessage"
				:actions="[{ label: 'Retry', icon: 'i-lucide-refresh-cw', color: 'neutral', variant: 'outline', onClick: loadProcesses }]"
				class="flex-1"
			/>

			<div
				v-else-if="isLoading && processes.length === 0"
				class="space-y-3 p-4"
			>
				<div
					v-for="n in 8"
					:key="n"
					class="flex items-center gap-3"
				>
					<USkeleton class="h-4 w-12 rounded" />
					<div class="flex-1 space-y-1">
						<USkeleton class="h-4 w-1/3" />
						<USkeleton class="h-3 w-1/2" />
					</div>
					<USkeleton class="h-4 w-20" />
					<USkeleton class="h-8 w-28 rounded" />
				</div>
			</div>

			<UEmpty
				v-else-if="filteredProcesses.length === 0"
				icon="i-lucide-search-x"
				:title="searchQuery ? 'No processes match your filter' : 'No processes found'"
				:description="searchQuery
					? 'Try searching with a shorter PID or executable name.'
					: 'The process table returned no active items.'"
				:actions="searchQuery ? [{ label: 'Clear search', color: 'neutral', variant: 'outline', onClick: () => { searchQuery = ''; onlyMatchedInstalled = false; } }] : undefined"
				class="flex-1"
			/>

			<UTable
				v-else
				:data="filteredProcesses"
				:columns="columns"
				:loading="isLoading"
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
		</template>
	</UDashboardPanel>

	<!-- Confirmation Modal -->
	<UModal
		v-model:open="confirmKillModalOpen"
		title="Close this program?"
		:description="`Close PID ${processToKill?.pid} (${processToKill?.name})?`"
		:dismissible="!isTerminating"
	>
		<template #body>
			<div
				v-if="processToKill"
				class="space-y-3 text-xs"
			>
				<UAlert
					color="warning"
					variant="subtle"
					icon="i-lucide-triangle-alert"
					title="Unsaved data warning"
					description="Tuntas first asks the program to close normally. If it does not respond within a few seconds, it is ended, and unsaved work in it is lost."
				/>

				<div class="p-3 rounded-lg bg-elevated border border-default font-mono space-y-1">
					<p class="font-semibold text-highlighted">
						{{ processToKill.name }} (PID #{{ processToKill.pid }})
					</p>
					<p class="text-muted break-all">
						{{ processToKill.exePath || 'System Process' }}
					</p>
				</div>
			</div>
		</template>

		<template #footer>
			<div class="flex w-full justify-end gap-2">
				<UButton
					label="No, Cancel"
					color="neutral"
					variant="outline"
					:disabled="isTerminating"
					@click="confirmKillModalOpen = false"
				/>
				<UButton
					label="Yes, close it"
					color="error"
					variant="solid"
					:loading="isTerminating"
					@click="confirmKillProcess"
				/>
			</div>
		</template>
	</UModal>
</template>
