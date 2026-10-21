<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui';
import type { DeletionReport, JunkCategoryItem } from '~/composables/useUninstallerBackend';
import { errorText, formatBytes, useUninstallerBackend } from '~/composables/useUninstallerBackend';
import { h, resolveComponent } from 'vue';

const UCheckbox = resolveComponent('UCheckbox');
const UButton = resolveComponent('UButton');
const toast = useToast();

const { scanSystemJunk, cleanSystemJunk, openInExplorer } = useUninstallerBackend();

const categories = ref<JunkCategoryItem[]>([]);
const rowSelection = ref<Record<string, boolean>>({});
const isScanning = ref(false);
const isCleaning = ref(false);
const errorMessage = ref<string | null>(null);
const cleanReport = ref<DeletionReport | null>(null);
const confirmModalOpen = ref(false);

const selectedCategories = computed(() => {
	return categories.value.filter(cat => rowSelection.value[cat.id]);
});

const totalSelectedBytes = computed(() => {
	return selectedCategories.value.reduce((sum, cat) => sum + cat.totalBytes, 0);
});

const cleanDescription = computed(() => {
	if (!cleanReport.value) {
		return undefined;
	}
	const lines = [...cleanReport.value.errors];
	if (cleanReport.value.skippedInUse > 0) {
		lines.unshift(`${cleanReport.value.skippedInUse.toLocaleString()} file(s) were open in another program or protected, so they stayed in place.`);
	}
	return lines.join('\n') || undefined;
});

const grandTotalBytes = computed(() => {
	return categories.value.reduce((acc, c) => acc + c.totalBytes, 0);
});

const grandTotalFiles = computed(() => {
	return categories.value.reduce((acc, c) => acc + c.fileCount, 0);
});

const columns: TableColumn<JunkCategoryItem>[] = [
	{
		id: 'select',
		header: ({ table }) => h(UCheckbox, {
			'modelValue': table.getIsSomePageRowsSelected() ? 'indeterminate' : table.getIsAllPageRowsSelected(),
			'onUpdate:modelValue': (v: boolean | 'indeterminate') => table.toggleAllPageRowsSelected(Boolean(v)),
			'aria-label': 'Select all categories'
		}),
		cell: ({ row }) => h(UCheckbox, {
			'modelValue': row.getIsSelected(),
			'onUpdate:modelValue': (v: boolean | 'indeterminate') => row.toggleSelected(Boolean(v)),
			'onClick': (e: Event) => e.stopPropagation(),
			'aria-label': `Select ${row.original.category}`
		}),
		meta: { class: { th: 'w-10', td: 'w-10' } }
	},
	{
		accessorKey: 'category',
		header: 'Category / Location',
		cell: ({ row }) => {
			const cat = row.original;
			return h('div', { class: 'min-w-0 py-1' }, [
				h('p', { class: 'font-medium text-highlighted' }, cat.category),
				h('p', { class: 'font-mono text-xs text-muted truncate mt-0.5' }, cat.path),
				cat.note ? h('p', { class: 'text-xs text-warning mt-1' }, cat.note) : null
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'fileCount',
		header: 'Files',
		cell: ({ row }) => `${row.original.fileCount.toLocaleString()} items`,
		meta: { class: { th: 'text-right w-32', td: 'text-right tabular-nums text-muted text-xs whitespace-nowrap w-32' } }
	},
	{
		accessorKey: 'totalBytes',
		header: 'Reclaimable',
		cell: ({ row }) => formatBytes(row.original.totalBytes),
		meta: { class: { th: 'text-right w-32', td: 'text-right tabular-nums font-semibold text-highlighted whitespace-nowrap w-32' } }
	},
	{
		id: 'actions',
		header: 'Explorer',
		meta: { class: { th: 'text-right w-24', td: 'text-right whitespace-nowrap w-24' } },
		cell: ({ row }) => {
			const cat = row.original;
			return h(UButton, {
				'icon': 'i-lucide-folder-open',
				'color': 'neutral',
				'variant': 'ghost',
				'size': 'xs',
				'aria-label': `Open ${cat.category} in Explorer`,
				'onClick': () => handleOpenPath(cat.path)
			});
		}
	}
];

async function runScan() {
	isScanning.value = true;
	errorMessage.value = null;
	try {
		const res = await scanSystemJunk();
		categories.value = res;
		const sel: Record<string, boolean> = {};
		for (const cat of res) {
			if (cat.totalBytes > 0) {
				sel[cat.id] = true;
			}
		}
		rowSelection.value = sel;
	}
	catch (err: unknown) {
		errorMessage.value = errorText(err);
	}
	finally {
		isScanning.value = false;
	}
}

function requestClean() {
	if (selectedCategories.value.length === 0) return;
	confirmModalOpen.value = true;
}

async function confirmClean() {
	confirmModalOpen.value = false;
	const ids = selectedCategories.value.map(c => c.id);
	if (ids.length === 0) return;

	isCleaning.value = true;
	errorMessage.value = null;
	cleanReport.value = null;
	try {
		const report = await cleanSystemJunk(ids);
		cleanReport.value = report;
		toast.add({
			title: 'Clean complete',
			description: `Deleted ${report.removedCount.toLocaleString()} files and freed ${formatBytes(report.freedBytes)}.`,
			color: report.errors.length === 0 ? 'success' : 'warning'
		});
		await runScan();
	}
	catch (err: unknown) {
		errorMessage.value = errorText(err);
		toast.add({
			title: 'Purge failed',
			description: errorMessage.value,
			color: 'error'
		});
	}
	finally {
		isCleaning.value = false;
	}
}

async function handleOpenPath(path: string) {
	try {
		await openInExplorer(path);
	}
	catch (err: unknown) {
		toast.add({
			title: 'Could not open folder',
			description: errorText(err),
			color: 'error'
		});
	}
}

onMounted(runScan);
</script>

<template>
	<UDashboardPanel id="cleaner">
		<template #header>
			<UDashboardNavbar title="Junk Files Cleaner">
				<template #leading>
					<UDashboardSidebarCollapse />
				</template>
				<template #trailing>
					<UBadge
						:label="formatBytes(grandTotalBytes)"
						color="neutral"
						variant="subtle"
					/>
				</template>
				<template #right>
					<UButton
						icon="i-lucide-refresh-cw"
						color="neutral"
						variant="ghost"
						:loading="isScanning"
						aria-label="Rescan junk files"
						@click="runScan"
					/>
					<UButton
						label="Purge Selected"
						icon="i-lucide-trash-2"
						color="error"
						variant="solid"
						:disabled="selectedCategories.length === 0 || isCleaning || isScanning"
						:loading="isCleaning"
						@click="requestClean"
					/>
				</template>
			</UDashboardNavbar>
		</template>

		<template #body>
			<div class="space-y-4">
				<!-- KPI Cards -->
				<div class="grid grid-cols-1 sm:grid-cols-3 gap-3">
					<UPageCard
						icon="i-lucide-hard-drive"
						title="Reclaimable Space"
						variant="subtle"
						:ui="{
							container: 'gap-y-1 p-3 sm:p-4',
							leading: 'p-2 rounded-full bg-primary/10 ring ring-inset ring-primary/25',
							leadingIcon: 'size-4 text-primary',
							title: 'font-normal text-muted text-xs uppercase'
						}"
					>
						<p class="text-xl font-semibold text-highlighted tabular-nums">
							{{ formatBytes(grandTotalBytes) }}
						</p>
						<p class="text-xs text-muted">
							Across {{ categories.length }} cache targets
						</p>
					</UPageCard>

					<UPageCard
						icon="i-lucide-file-stack"
						title="Temporary Files"
						variant="subtle"
						:ui="{
							container: 'gap-y-1 p-3 sm:p-4',
							leading: 'p-2 rounded-full bg-primary/10 ring ring-inset ring-primary/25',
							leadingIcon: 'size-4 text-primary',
							title: 'font-normal text-muted text-xs uppercase'
						}"
					>
						<p class="text-xl font-semibold text-highlighted tabular-nums">
							{{ grandTotalFiles.toLocaleString() }}
						</p>
						<p class="text-xs text-muted">
							Temporary files, logs, and caches
						</p>
					</UPageCard>

					<UPageCard
						icon="i-lucide-file-lock"
						title="Files in use"
						variant="subtle"
						:ui="{
							container: 'gap-y-1 p-3 sm:p-4',
							leading: 'p-2 rounded-full bg-warning/10 ring ring-inset ring-warning/25',
							leadingIcon: 'size-4 text-warning',
							title: 'font-normal text-muted text-xs uppercase'
						}"
					>
						<p class="text-xl font-semibold text-highlighted tabular-nums">
							{{ cleanReport ? cleanReport.skippedInUse.toLocaleString() : '-' }}
						</p>
						<p class="text-xs text-muted">
							{{ cleanReport ? 'Skipped because another program had them open' : 'Open files are skipped, never forced' }}
						</p>
					</UPageCard>
				</div>

				<!-- Clean Feedback Alert -->
				<UAlert
					v-if="cleanReport"
					color="success"
					variant="subtle"
					icon="i-lucide-circle-check"
					:title="`Deleted ${cleanReport.removedCount.toLocaleString()} files, freed ${formatBytes(cleanReport.freedBytes)}.`"
					:description="cleanDescription"
					:ui="{ description: 'whitespace-pre-line' }"
					close
					@update:open="cleanReport = null"
				/>

				<!-- Error Alert -->
				<UAlert
					v-if="errorMessage"
					color="error"
					variant="subtle"
					icon="i-lucide-triangle-alert"
					title="Cleaner error"
					:description="errorMessage"
					close
					@update:open="errorMessage = null"
				/>

				<!-- Scanning State Skeleton -->
				<div
					v-if="isScanning && categories.length === 0"
					class="space-y-3 p-4"
				>
					<div
						v-for="n in 5"
						:key="n"
						class="flex items-center gap-3"
					>
						<USkeleton class="size-5 rounded" />
						<div class="flex-1 space-y-1">
							<USkeleton class="h-4 w-1/3" />
							<USkeleton class="h-3 w-1/2" />
						</div>
						<USkeleton class="h-4 w-16" />
						<USkeleton class="h-4 w-20" />
					</div>
				</div>

				<UEmpty
					v-else-if="categories.length === 0"
					icon="i-lucide-trash-2"
					title="No temporary cache locations identified"
					description="Your Windows system temporary cache folders could not be queried."
					class="py-12"
				/>

				<!-- Table of Targets -->
				<div
					v-else
					class="space-y-3"
				>
					<div class="flex items-center justify-between px-1">
						<span class="text-xs font-medium text-muted">
							Selected: {{ selectedCategories.length }} of {{ categories.length }} categories ({{ formatBytes(totalSelectedBytes) }})
						</span>
					</div>

					<UTable
						v-model:row-selection="rowSelection"
						:data="categories"
						:columns="columns"
						:get-row-id="(row: JunkCategoryItem) => row.id"
						:loading="isScanning"
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
		</template>
	</UDashboardPanel>

	<!-- Confirmation Modal -->
	<UModal
		v-model:open="confirmModalOpen"
		title="Purge selected temporary folders?"
		:description="`You are about to delete contents of ${selectedCategories.length} temporary cache directories (${formatBytes(totalSelectedBytes)}).`"
		:dismissible="!isCleaning"
	>
		<template #body>
			<div class="space-y-3 text-xs">
				<p class="text-muted">
					Files inside these folders are deleted permanently; the folders themselves stay. Anything another program has open is skipped, so it is safe to run while you work.
				</p>
				<ul class="max-h-48 overflow-y-auto divide-y divide-default border border-default rounded-md">
					<li
						v-for="cat in selectedCategories"
						:key="cat.id"
						class="flex items-center justify-between p-2.5"
					>
						<span class="font-medium text-highlighted truncate">{{ cat.category }}</span>
						<span class="font-mono text-muted tabular-nums shrink-0">{{ formatBytes(cat.totalBytes) }}</span>
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
					:disabled="isCleaning"
					@click="confirmModalOpen = false"
				/>
				<UButton
					:label="`Yes, Purge ${formatBytes(totalSelectedBytes)}`"
					color="error"
					variant="solid"
					:loading="isCleaning"
					@click="confirmClean"
				/>
			</div>
		</template>
	</UModal>
</template>
