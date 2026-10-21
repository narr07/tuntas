<script setup lang="ts">
import type { InstalledApp, LeftoverItem } from '~/composables/useUninstallerBackend';
import { formatInstallDate, formatKb } from '~/composables/useUninstallerBackend';
import { useClipboard } from '@vueuse/core';

const props = defineProps<{
	app: InstalledApp;
	iconSrc?: string;
	isNativeUninstalling: boolean;
	hasCompletedNativeUninstall: boolean;
	isScanningLeftovers: boolean;
	hasScannedCurrentApp: boolean;
	leftovers: LeftoverItem[];
	selectedLeftoverIds: Set<string>;
	isDeletingLeftovers: boolean;
	isBatchRunning: boolean;
}>();

const emit = defineEmits<{
	(e: 'close'): void;
	(e: 'uninstall', preferQuiet: boolean): void;
	(e: 'rescan'): void;
	(e: 'open-folder', path: string): void;
	(e: 'toggle-leftover', id: string): void;
	(e: 'select-all-leftovers', value: boolean): void;
	(e: 'request-delete-leftovers'): void;
}>();

const toast = useToast();
const { copy } = useClipboard();

const isBusy = computed(() => props.isNativeUninstalling || props.isScanningLeftovers || props.isBatchRunning);
const showLeftovers = computed(() => props.hasCompletedNativeUninstall || props.isScanningLeftovers || props.hasScannedCurrentApp);

async function copyRegistryPath() {
	await copy(props.app.registryPath);
	toast.add({ title: 'Registry path copied', icon: 'i-lucide-clipboard-check', color: 'success' });
}
</script>

<template>
	<UDashboardPanel id="program-inspector">
		<template #header>
			<UDashboardNavbar
				:title="app.displayName"
				:toggle="false"
			>
				<template #leading>
					<UButton
						icon="i-lucide-x"
						color="neutral"
						variant="ghost"
						class="-ms-1.5"
						aria-label="Close details"
						@click="emit('close')"
					/>
				</template>
			</UDashboardNavbar>
		</template>

		<template #body>
			<div class="flex items-start gap-4">
				<UAvatar
					:src="iconSrc"
					:alt="app.displayName"
					size="2xl"
					class="rounded-lg bg-elevated ring ring-default"
				/>
				<div class="min-w-0 flex-1">
					<p class="text-lg font-semibold text-highlighted break-words">
						{{ app.displayName }}
					</p>
					<p class="text-sm text-muted">
						{{ app.publisher || 'Unknown publisher' }}
					</p>
					<div class="mt-2 flex flex-wrap items-center gap-1.5">
						<UBadge
							:label="app.architecture"
							color="neutral"
							variant="subtle"
						/>
						<UBadge
							v-if="app.displayVersion"
							:label="`v${app.displayVersion}`"
							color="neutral"
							variant="outline"
						/>
						<UBadge
							v-if="app.systemComponent"
							label="System component"
							color="warning"
							variant="subtle"
						/>
					</div>
				</div>
			</div>

			<div class="flex flex-wrap items-center gap-2">
				<UButton
					label="Uninstall"
					icon="i-lucide-trash-2"
					color="error"
					variant="solid"
					:disabled="!app.uninstallString || isBusy"
					:loading="isNativeUninstalling"
					@click="emit('uninstall', false)"
				/>
				<UTooltip
					v-if="app.quietUninstallString"
					text="Run the uninstaller without its wizard"
				>
					<UButton
						label="Quiet uninstall"
						icon="i-lucide-volume-x"
						color="error"
						variant="outline"
						:disabled="isBusy"
						@click="emit('uninstall', true)"
					/>
				</UTooltip>
			</div>
			<p
				v-if="!app.uninstallString"
				class="text-xs text-muted -mt-2"
			>
				This entry has no uninstall command in the registry. Use Forced Uninstall to remove it.
			</p>

			<UCard :ui="{ body: 'p-0 sm:p-0' }">
				<dl class="divide-y divide-default text-sm">
					<div class="grid grid-cols-3 gap-3 px-4 py-3">
						<dt class="text-muted">
							Size
						</dt>
						<dd class="col-span-2 text-highlighted tabular-nums">
							{{ formatKb(app.estimatedSizeKb) }}
						</dd>
					</div>
					<div class="grid grid-cols-3 gap-3 px-4 py-3">
						<dt class="text-muted">
							Installed
						</dt>
						<dd class="col-span-2 text-highlighted">
							{{ formatInstallDate(app.installDate) }}
						</dd>
					</div>
					<div class="grid grid-cols-3 gap-3 px-4 py-3">
						<dt class="text-muted">
							Location
						</dt>
						<dd class="col-span-2 flex items-start justify-between gap-2 min-w-0">
							<span class="font-mono text-xs text-highlighted break-all">
								{{ app.installLocation || 'Not recorded' }}
							</span>
							<UButton
								v-if="app.installLocation"
								icon="i-lucide-folder-open"
								color="neutral"
								variant="ghost"
								size="sm"
								aria-label="Open install folder in Explorer"
								class="-my-1"
								@click="emit('open-folder', app.installLocation)"
							/>
						</dd>
					</div>
					<div class="grid grid-cols-3 gap-3 px-4 py-3">
						<dt class="text-muted">
							Registry key
						</dt>
						<dd class="col-span-2 flex items-start justify-between gap-2 min-w-0">
							<span class="font-mono text-xs text-highlighted break-all">
								{{ app.registryPath }}
							</span>
							<UButton
								icon="i-lucide-copy"
								color="neutral"
								variant="ghost"
								size="sm"
								aria-label="Copy registry path"
								class="-my-1"
								@click="copyRegistryPath"
							/>
						</dd>
					</div>
				</dl>
			</UCard>

			<LeftoverList
				v-if="showLeftovers"
				:leftovers="leftovers"
				:selected-ids="selectedLeftoverIds"
				:is-scanning="isScanningLeftovers"
				:has-scanned="hasScannedCurrentApp"
				:is-deleting="isDeletingLeftovers"
				@toggle="(id) => emit('toggle-leftover', id)"
				@select-all="(value) => emit('select-all-leftovers', value)"
				@request-delete="emit('request-delete-leftovers')"
				@rescan="emit('rescan')"
			/>
		</template>
	</UDashboardPanel>
</template>
