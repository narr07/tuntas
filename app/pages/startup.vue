<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui';
import type { StartupEntry } from '~/composables/useUninstallerBackend';
import { errorText, useUninstallerBackend } from '~/composables/useUninstallerBackend';
import { h, resolveComponent } from 'vue';

const UButton = resolveComponent('UButton');
const UBadge = resolveComponent('UBadge');
const toast = useToast();

const { listStartupEntries, setStartupEntryEnabled } = useUninstallerBackend();

const entries = ref<StartupEntry[]>([]);
const isLoading = ref(false);
const errorMessage = ref<string | null>(null);
const searchQuery = ref('');
const togglingId = ref<string | null>(null);

const filteredEntries = computed(() => {
	const q = searchQuery.value.trim().toLowerCase();
	if (!q) {
		return entries.value;
	}
	return entries.value.filter(
		e =>
			e.name.toLowerCase().includes(q)
			|| e.command.toLowerCase().includes(q)
			|| e.location.toLowerCase().includes(q)
	);
});

const enabledCount = computed(() => entries.value.filter(e => e.enabled).length);

const columns: TableColumn<StartupEntry>[] = [
	{
		accessorKey: 'name',
		header: 'Program / Entry',
		cell: ({ row }) => {
			const entry = row.original;
			return h('div', { class: 'flex items-center gap-2.5 py-1 min-w-0' }, [
				h('span', {
					class: [
						'size-2 rounded-full shrink-0',
						entry.enabled ? 'bg-success' : 'bg-muted'
					]
				}),
				h('div', { class: 'min-w-0 flex-1' }, [
					h('p', { class: 'font-medium text-highlighted truncate' }, entry.name),
					h('p', { class: 'font-mono text-xs text-muted truncate mt-0.5' }, entry.command)
				])
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'location',
		header: 'Source Location',
		cell: ({ row }) => {
			const loc = row.original.location;
			return h('div', { class: 'flex items-center gap-1.5' }, [
				h(UBadge, {
					label: row.original.scope === 'user' ? 'User' : 'All users',
					color: 'neutral',
					variant: 'subtle',
					size: 'sm'
				}),
				h('span', { class: 'font-mono text-xs text-muted truncate max-w-xs' }, loc)
			]);
		},
		meta: { class: { th: 'hidden md:table-cell w-72', td: 'hidden md:table-cell w-72' } }
	},
	{
		accessorKey: 'enabled',
		header: 'Status',
		cell: ({ row }) => {
			const enabled = row.original.enabled;
			return h(UBadge, {
				label: enabled ? 'Enabled' : 'Disabled',
				color: enabled ? 'success' : 'neutral',
				variant: 'subtle',
				size: 'sm'
			});
		},
		meta: { class: { th: 'w-24', td: 'w-24 whitespace-nowrap' } }
	},
	{
		id: 'actions',
		header: 'Action',
		meta: { class: { th: 'text-right w-28', td: 'text-right whitespace-nowrap w-28' } },
		cell: ({ row }) => {
			const entry = row.original;
			const isToggling = togglingId.value === entry.id;

			return h(UButton, {
				label: entry.enabled ? 'Disable' : 'Enable',
				color: entry.enabled ? 'warning' : 'success',
				variant: entry.enabled ? 'outline' : 'subtle',
				size: 'xs',
				loading: isToggling,
				disabled: Boolean(togglingId.value),
				onClick: () => toggleEntry(entry)
			});
		}
	}
];

async function loadEntries() {
	isLoading.value = true;
	errorMessage.value = null;
	try {
		entries.value = await listStartupEntries();
	}
	catch (err: unknown) {
		errorMessage.value = errorText(err);
	}
	finally {
		isLoading.value = false;
	}
}

async function toggleEntry(entry: StartupEntry) {
	togglingId.value = entry.id;
	try {
		const targetState = !entry.enabled;
		const msg = await setStartupEntryEnabled(entry, targetState);
		toast.add({
			title: targetState ? 'Startup entry enabled' : 'Startup entry disabled',
			description: msg || `${entry.name} startup status changed.`,
			color: 'success'
		});
		await loadEntries();
	}
	catch (err: unknown) {
		toast.add({
			title: 'Could not change startup entry',
			description: errorText(err),
			color: 'error'
		});
	}
	finally {
		togglingId.value = null;
	}
}

onMounted(loadEntries);
</script>

<template>
	<UDashboardPanel id="startup">
		<template #header>
			<UDashboardNavbar title="Startup Manager">
				<template #leading>
					<UDashboardSidebarCollapse />
				</template>
				<template #trailing>
					<UBadge
						:label="`${enabledCount} enabled of ${entries.length}`"
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
						aria-label="Reload startup entries"
						@click="loadEntries"
					/>
				</template>
			</UDashboardNavbar>

			<UDashboardToolbar>
				<div class="flex w-full items-center justify-between gap-3 py-1">
					<UInput
						v-model="searchQuery"
						icon="i-lucide-search"
						placeholder="Filter by entry name, binary path, or registry hive..."
						class="w-full sm:w-80"
						aria-label="Search startup items"
					/>
				</div>
			</UDashboardToolbar>
		</template>

		<template #body>
			<UEmpty
				v-if="errorMessage"
				icon="i-lucide-triangle-alert"
				title="Could not query Windows startup entries"
				:description="errorMessage"
				:actions="[{ label: 'Retry', icon: 'i-lucide-refresh-cw', color: 'neutral', variant: 'outline', onClick: loadEntries }]"
				class="flex-1"
			/>

			<div
				v-else-if="isLoading && entries.length === 0"
				class="space-y-3 p-4"
			>
				<div
					v-for="n in 6"
					:key="n"
					class="flex items-center gap-3"
				>
					<USkeleton class="size-4 rounded-full" />
					<div class="flex-1 space-y-1">
						<USkeleton class="h-4 w-1/4" />
						<USkeleton class="h-3 w-1/2" />
					</div>
					<USkeleton class="h-4 w-20" />
					<USkeleton class="h-8 w-16 rounded" />
				</div>
			</div>

			<UEmpty
				v-else-if="filteredEntries.length === 0"
				icon="i-lucide-power-off"
				:title="searchQuery ? 'No startup items match your filter' : 'No startup entries registered'"
				:description="searchQuery
					? 'Try searching with another keyword.'
					: 'No startup entries found in Run hives or Startup folders.'"
				:actions="searchQuery ? [{ label: 'Clear filter', color: 'neutral', variant: 'outline', onClick: () => { searchQuery = ''; } }] : undefined"
				class="flex-1"
			/>

			<UTable
				v-else
				:data="filteredEntries"
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
</template>
