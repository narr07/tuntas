<script setup lang="ts">
import type { DeletionReport, LeftoverItem, LockedItem, LockingProcess } from '~/composables/useUninstallerBackend';
import { errorText, useElevation, useUninstallerBackend } from '~/composables/useUninstallerBackend';

const props = defineProps<{
	locked: LockedItem[];
	items: LeftoverItem[];
}>();

const emit = defineEmits<{
	(e: 'resolved', report: DeletionReport): void;
	(e: 'scheduled'): void;
}>();

const open = defineModel<boolean>('open', { default: false });

const toast = useToast();
const { releaseLocks, deleteLeftoverItems, scheduleDeleteOnReboot } = useUninstallerBackend();
const { elevated } = useElevation();

const busy = ref<'close' | 'force' | 'reboot' | null>(null);
const warning = ref<string | null>(null);

const processes = computed<LockingProcess[]>(() => {
	const byPid = new Map<number, LockingProcess>();
	for (const item of props.locked) {
		for (const proc of item.processes) {
			byPid.set(proc.pid, proc);
		}
	}
	return [...byPid.values()];
});

const restartsExplorer = computed(() => processes.value.some(p => p.name.toLowerCase() === 'explorer.exe'));

watch(open, (value) => {
	if (value) {
		warning.value = null;
	}
});

async function releaseAndRetry(force: boolean) {
	busy.value = force ? 'force' : 'close';
	warning.value = null;
	try {
		const release = await releaseLocks(processes.value, force);
		if (release.stillRunning.length > 0) {
			const names = release.stillRunning.map(p => p.appName || p.name).join(', ');
			warning.value = force
				? `${names} could not be closed. ${release.errors.join(' ')}`
				: `${names} did not close. It may be waiting for you to save something. Switch to it, or use Force close.`;
		}
		const lockedPaths = new Set(props.locked.map(l => l.path));
		const report = await deleteLeftoverItems(props.items.filter(i => lockedPaths.has(i.path)));
		emit('resolved', report);
	}
	catch (err: unknown) {
		warning.value = errorText(err);
	}
	finally {
		busy.value = null;
	}
}

async function deleteOnRestart() {
	busy.value = 'reboot';
	warning.value = null;
	try {
		const report = await scheduleDeleteOnReboot(props.locked.map(l => l.path));
		if (report.errors.length > 0) {
			warning.value = report.errors.join('\n');
			return;
		}
		toast.add({
			title: 'Scheduled for the next restart',
			description: `${report.scheduled} item(s) will be removed when Windows starts again.`,
			color: 'success',
			icon: 'i-lucide-calendar-clock'
		});
		emit('scheduled');
		open.value = false;
	}
	catch (err: unknown) {
		warning.value = errorText(err);
	}
	finally {
		busy.value = null;
	}
}
</script>

<template>
	<UModal
		v-model:open="open"
		title="Some files are still in use"
		description="Windows will not delete a file while another program has it open."
		:dismissible="!busy"
		:ui="{ content: 'sm:max-w-xl' }"
	>
		<template #body>
			<div class="space-y-4">
				<div class="space-y-2">
					<p class="text-xs font-medium uppercase tracking-wide text-muted">
						Holding the files
					</p>
					<div class="flex flex-wrap gap-2">
						<UBadge
							v-for="proc in processes"
							:key="proc.pid"
							:label="`${proc.appName || proc.name} · PID ${proc.pid}`"
							:icon="proc.serviceName ? 'i-lucide-cog' : 'i-lucide-app-window'"
							color="neutral"
							variant="subtle"
						/>
					</div>
				</div>

				<ul class="max-h-48 overflow-y-auto divide-y divide-default rounded-md ring ring-default">
					<li
						v-for="item in locked"
						:key="item.path"
						class="px-3 py-2 space-y-1"
					>
						<p class="font-mono text-xs text-highlighted break-all">
							{{ item.path }}
						</p>
						<p class="text-xs text-muted">
							{{ item.files.length }} file(s) still in place{{ item.files.length >= 20 ? ' or more' : '' }}
						</p>
					</li>
				</ul>

				<UAlert
					v-if="restartsExplorer"
					color="info"
					variant="subtle"
					icon="i-lucide-info"
					title="Windows Explorer has the files open"
					description="Closing it restarts the taskbar and desktop. Open folder windows will close."
				/>

				<UAlert
					v-if="warning"
					color="warning"
					variant="subtle"
					icon="i-lucide-triangle-alert"
					:description="warning"
					:ui="{ description: 'whitespace-pre-line' }"
				/>
			</div>
		</template>

		<template #footer>
			<div class="flex w-full flex-wrap items-center justify-between gap-2">
				<UTooltip
					:text="elevated ? 'Removes the files before any program can open them on the next boot' : 'Needs administrator rights'"
				>
					<UButton
						label="Delete on restart"
						icon="i-lucide-calendar-clock"
						color="neutral"
						variant="ghost"
						:disabled="!elevated || Boolean(busy)"
						:loading="busy === 'reboot'"
						@click="deleteOnRestart"
					/>
				</UTooltip>
				<div class="flex flex-wrap gap-2">
					<UButton
						label="Skip"
						color="neutral"
						variant="outline"
						:disabled="Boolean(busy)"
						@click="open = false"
					/>
					<UButton
						label="Force close"
						color="error"
						variant="subtle"
						:disabled="Boolean(busy)"
						:loading="busy === 'force'"
						@click="releaseAndRetry(true)"
					/>
					<UButton
						label="Close apps and retry"
						icon="i-lucide-refresh-cw"
						color="primary"
						variant="solid"
						:disabled="Boolean(busy)"
						:loading="busy === 'close'"
						@click="releaseAndRetry(false)"
					/>
				</div>
			</div>
		</template>
	</UModal>
</template>
