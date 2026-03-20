export type ToastType = 'success' | 'error' | 'info' | 'warning';

export interface Toast {
	id: string;
	type: ToastType;
	message: string;
	duration: number;
}

let toasts = $state<Toast[]>([]);
let counter = 0;
const timers = new Map<string, ReturnType<typeof setTimeout>>();

export function getToasts() {
	return toasts;
}

export function addToast(type: ToastType, message: string, duration = 4000) {
	const id = `toast-${++counter}`;
	toasts.push({ id, type, message, duration });

	if (duration > 0) {
		timers.set(
			id,
			setTimeout(() => removeToast(id), duration)
		);
	}
}

export function removeToast(id: string) {
	const timer = timers.get(id);
	if (timer) {
		clearTimeout(timer);
		timers.delete(id);
	}
	const idx = toasts.findIndex((t) => t.id === id);
	if (idx !== -1) toasts.splice(idx, 1);
}

export function success(message: string, duration?: number) {
	addToast('success', message, duration);
}

export function error(message: string, duration?: number) {
	addToast('error', message, duration ?? 6000);
}

export function info(message: string, duration?: number) {
	addToast('info', message, duration);
}

export function warning(message: string, duration?: number) {
	addToast('warning', message, duration ?? 5000);
}
