/**
 * Make the platform's "back" gesture close an overlay (drawer, sheet, palette) instead of
 * leaving the page — what Android users expect from an installed app.
 *
 * Uses `CloseWatcher` where it exists (Chrome 120+); otherwise pushes a history entry and
 * listens for `popstate`. Call the returned function when the overlay closes by other means
 * so the history entry (if any) is consumed.
 */
export function watchClose(onClose: () => void): () => void {
	if (typeof window === 'undefined') return () => {};

	const CW = (window as unknown as { CloseWatcher?: new () => CloseWatcherLike }).CloseWatcher;
	if (CW) {
		let watcher: CloseWatcherLike | null;
		try {
			watcher = new CW();
		} catch {
			watcher = null; // too many watchers without a user gesture; fall back below
		}
		if (watcher) {
			const w = watcher;
			let closedByUs = false;
			w.onclose = () => {
				if (!closedByUs) onClose();
			};
			return () => {
				closedByUs = true;
				w.destroy();
			};
		}
	}

	const marker = { svrctlrsOverlay: Date.now() };
	history.pushState(marker, '');
	let done = false;
	const onPop = () => {
		if (done) return;
		done = true;
		window.removeEventListener('popstate', onPop);
		onClose();
	};
	window.addEventListener('popstate', onPop);
	return () => {
		if (done) return;
		done = true;
		window.removeEventListener('popstate', onPop);
		// Our entry is still on top: step back over it without triggering onClose
		if (
			(history.state as { svrctlrsOverlay?: number } | null)?.svrctlrsOverlay ===
			marker.svrctlrsOverlay
		) {
			history.back();
		}
	};
}

interface CloseWatcherLike {
	onclose: (() => void) | null;
	destroy(): void;
}
