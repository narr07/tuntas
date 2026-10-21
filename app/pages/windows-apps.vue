<script setup lang="ts">
import type { TableColumn } from '@nuxt/ui';
import type { WindowsStoreApp } from '~/composables/useUninstallerBackend';
import { useUninstallerBackend } from '~/composables/useUninstallerBackend';
import { h, resolveComponent } from 'vue';

const UButton = resolveComponent('UButton');
const UBadge = resolveComponent('UBadge');
const UIcon = resolveComponent('UIcon');
const toast = useToast();

const { listWindowsStoreApps, removeWindowsStoreApp, openInExplorer } = useUninstallerBackend();

const apps = ref<WindowsStoreApp[]>([]);
const isLoading = ref(false);
const errorMessage = ref<string | null>(null);
const searchQuery = ref('');
const hideNonRemovable = ref(true);

const removingPkg = ref<string | null>(null);
const confirmModalOpen = ref(false);
const packageToRemove = ref<WindowsStoreApp | null>(null);

const filteredApps = computed(() => {
	const q = searchQuery.value.trim().toLowerCase();
	return apps.value.filter((pkg) => {
		if (hideNonRemovable.value && pkg.nonRemovable) {
			return false;
		}
		if (!q) {
			return true;
		}
		return (
			pkg.name.toLowerCase().includes(q)
			|| pkg.packageFullName.toLowerCase().includes(q)
			|| pkg.publisher.toLowerCase().includes(q)
		);
	});
});

const columns: TableColumn<WindowsStoreApp>[] = [
	{
		accessorKey: 'name',
		header: 'Appx Package',
		cell: ({ row }) => {
			const pkg = row.original;
			return h('div', { class: 'flex items-center gap-3 py-1 min-w-0' }, [
				h(
					'div',
					{
						class: 'size-8 rounded-lg bg-primary/10 text-primary border border-primary/25 flex items-center justify-center shrink-0'
					},
					[
						h(UIcon, {
							name: 'i-lucide-box',
							class: 'size-4'
						})
					]
				),
				h('div', { class: 'min-w-0 flex-1' }, [
					h('div', { class: 'flex items-center gap-1.5' }, [
						h('p', { class: 'font-medium text-highlighted truncate' }, pkg.name),
						pkg.nonRemovable
							? h(UBadge, { label: 'Protected', color: 'warning', variant: 'subtle', size: 'sm' })
							: null
					]),
					h('p', { class: 'font-mono text-xs text-muted truncate mt-0.5' }, pkg.packageFullName)
				])
			]);
		},
		meta: { class: { td: 'max-w-0 w-full' } }
	},
	{
		accessorKey: 'version',
		header: 'Version',
		cell: ({ row }) => row.original.version || '-',
		meta: { class: { th: 'w-32', td: 'font-mono text-xs text-muted whitespace-nowrap w-32' } }
	},
	{
		accessorKey: 'installLocation',
		header: 'Install Location',
		cell: ({ row }) => {
			const loc = row.original.installLocation;
			if (!loc) return '-';
			return h('div', { class: 'flex items-center justify-between gap-2 max-w-xs' }, [
				h('span', { class: 'font-mono text-xs text-muted truncate' }, loc),
				h(UButton, {
					'icon': 'i-lucide-folder-open',
					'color': 'neutral',
					'variant': 'ghost',
					'size': 'xs',
					'aria-label': 'Open install folder',
					'onClick': () => handleOpenPath(loc)
				})
			]);
		},
		meta: { class: { th: 'hidden md:table-cell w-64', td: 'hidden md:table-cell w-64' } }
	},
	{
		id: 'actions',
		header: 'Action',
		meta: { class: { th: 'text-right w-28', td: 'text-right whitespace-nowrap w-28' } },
		cell: ({ row }) => {
			const pkg = row.original;
			const isRemoving = removingPkg.value === pkg.packageFullName;

			return h(UButton, {
				label: pkg.nonRemovable ? 'Protected' : 'Remove',
				icon: 'i-lucide-trash-2',
				color: pkg.nonRemovable ? 'neutral' : 'error',
				variant: pkg.nonRemovable ? 'outline' : 'subtle',
				size: 'xs',
				disabled: pkg.nonRemovable || Boolean(removingPkg.value),
				loading: isRemoving,
				onClick: () => requestRemovePackage(pkg)
			});
		}
	}
];

async function loadApps() {
	isLoading.value = true;
	errorMessage.value = null;
	try {
		apps.value = await listWindowsStoreApps();
	}
	catch (err: unknown) {
		errorMessage.value = err instanceof Error ? err.message : String(err);
	}
	finally {
		isLoading.value = false;
	}
}

function requestRemovePackage(pkg: WindowsStoreApp) {
	packageToRemove.value = pkg;
	confirmModalOpen.value = true;
}

async function confirmRemovePackage() {
	if (!packageToRemove.value) return;
	const pkg = packageToRemove.value;
	confirmModalOpen.value = false;
	removingPkg.value = pkg.packageFullName;
	try {
		const msg = await removeWindowsStoreApp(pkg.packageFullName);
		toast.add({
			title: 'Package removed',
			description: msg || `Successfully uninstalled ${pkg.name}.`,
			color: 'success'
		});
		await loadApps();
	}
	catch (err: unknown) {
		const errText = err instanceof Error ? err.message : String(err);
		toast.add({
			title: 'Uninstall failed',
			description: errText,
			color: 'error'
		});
	}
	finally {
		removingPkg.value = null;
		packageToRemove.value = null;
	}
}

async function handleOpenPath(path: string) {
	if (!path) return;
	try {
		await openInExplorer(path);
	}
	catch (err: unknown) {
		toast.add({
			title: 'Could not open folder',
			description: err instanceof Error ? err.message : String(err),
			color: 'error'
		});
	}
}

onMounted(loadApps);
</script>

<template>
	<UDashboardPanel id="windows-apps">
		<template #header>
			<UDashboardNavbar title="Windows Apps">
				<template #leading>
					<UDashboardSidebarCollapse />
				</template>
				<template #trailing>
					<UBadge
						:label="`${filteredApps.length} / ${apps.length}`"
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
						aria-label="Reload Appx packages"
						@click="loadApps"
					/>
				</template>
			</UDashboardNavbar>

			<UDashboardToolbar>
				<div class="flex w-full flex-wrap items-center justify-between gap-3 py-1">
					<UInput
						v-model="searchQuery"
						icon="i-lucide-search"
						placeholder="Search Appx (e.g. Xbox, Bing, Store)..."
						class="w-full sm:w-72"
						aria-label="Search Windows Store apps"
					/>

					<USwitch
						v-model="hideNonRemovable"
						label="Hide protected system packages"
						size="sm"
					/>
				</div>
			</UDashboardToolbar>
		</template>

		<template #body>
			<UEmpty
				v-if="errorMessage"
				icon="i-lucide-triangle-alert"
				title="Could not query Appx packages"
				:description="errorMessage"
				:actions="[{ label: 'Retry', icon: 'i-lucide-refresh-cw', color: 'neutral', variant: 'outline', onClick: loadApps }]"
				class="flex-1"
			/>

			<div
				v-else-if="isLoading && apps.length === 0"
				class="space-y-3 p-4"
			>
				<div
					v-for="n in 8"
					:key="n"
					class="flex items-center gap-3"
				>
					<USkeleton class="size-8 rounded-lg" />
					<div class="flex-1 space-y-1">
						<USkeleton class="h-4 w-1/3" />
						<USkeleton class="h-3 w-1/2" />
					</div>
					<USkeleton class="h-4 w-20" />
					<USkeleton class="h-8 w-16 rounded" />
				</div>
			</div>

			<UEmpty
				v-else-if="filteredApps.length === 0"
				icon="i-lucide-package-open"
				:title="searchQuery ? 'No packages match your search' : 'No Appx packages found'"
				:description="searchQuery
					? 'Try searching with a shorter keyword or uncheck Hide protected system packages.'
					: 'Windows returned an empty package list.'"
				:actions="searchQuery ? [{ label: 'Clear search', color: 'neutral', variant: 'outline', onClick: () => { searchQuery = ''; hideNonRemovable = false; } }] : undefined"
				class="flex-1"
			/>

			<UTable
				v-else
				:data="filteredApps"
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
		v-model:open="confirmModalOpen"
		title="Uninstall Windows Store Package?"
		:description="`Are you sure you want to remove ${packageToRemove?.name}?`"
	>
		<template #body>
			<div
				v-if="packageToRemove"
				class="space-y-2 text-xs"
			>
				<p class="text-muted">
					This action executes Windows PowerShell Remove-AppxPackage for your current user profile.
				</p>
				<div class="p-3 rounded-lg bg-elevated border border-default font-mono space-y-1">
					<p class="font-semibold text-highlighted break-all">
						{{ packageToRemove.name }}
					</p>
					<p class="text-muted break-all">
						{{ packageToRemove.packageFullName }}
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
					@click="confirmModalOpen = false"
				/>
				<UButton
					label="Yes, Uninstall Package"
					color="error"
					variant="solid"
					@click="confirmRemovePackage"
				/>
			</div>
		</template>
	</UModal>
</template>
