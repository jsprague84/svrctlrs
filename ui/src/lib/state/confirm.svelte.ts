/**
 * App-wide confirmation dialog, replacing `window.confirm()` (which cannot be styled, names
 * nothing, and misbehaves inside the Tauri WebView). `ConfirmDialog.svelte` renders the
 * pending request; `confirmDialog()` resolves with the user's answer.
 */

export interface ConfirmOptions {
	title: string;
	/** What happens if they confirm; name the thing being deleted. */
	message: string;
	confirmLabel?: string;
	cancelLabel?: string;
	/** Red confirm button for deletes and other one-way actions */
	destructive?: boolean;
}

interface Pending extends ConfirmOptions {
	resolve: (ok: boolean) => void;
}

let pending = $state<Pending | null>(null);

export function getPending(): Pending | null {
	return pending;
}

/** Ask the user; resolves `true` on confirm. A second request cancels the first. */
export function confirmDialog(options: ConfirmOptions): Promise<boolean> {
	pending?.resolve(false);
	return new Promise((resolve) => {
		pending = { ...options, resolve };
	});
}

export function answer(ok: boolean) {
	const p = pending;
	pending = null;
	p?.resolve(ok);
}
