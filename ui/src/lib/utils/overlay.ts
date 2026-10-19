/**
 * Keyboard behaviour shared by every dialog, sheet and side panel: Escape closes it, Tab and
 * Shift+Tab stay inside a modal one, focus moves in when it opens and goes back to where it
 * was when it closes. Used as a Svelte attachment:
 *
 *     <div role="dialog" aria-modal="true" {@attach overlay({ onEscape: () => onClose() })}>
 *
 * Pass callbacks as arrow functions, as above, so the attachment is not re-created whenever
 * a prop the callback reads changes.
 */

export const FOCUSABLE =
	'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), ' +
	'textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';

export interface OverlayOptions {
	/** Escape closes the overlay; without it Escape is left alone */
	onEscape?: () => void;
	/** Keep Tab inside (modal dialogs and sheets). Side panels pass false. Default true. */
	trap?: boolean;
	/** Move focus to the first control when opened. Default true. */
	autofocus?: boolean;
	/** Give focus back to whatever had it before. Default true. */
	restoreFocus?: boolean;
}

/**
 * Where Tab should go from `current` in a cycle over `items`, or null to let the browser
 * move focus itself. Only the ends wrap; focus outside the list is pulled back in.
 */
export function wrapFocus<T>(items: T[], current: T | null, backwards: boolean): T | null {
	if (items.length === 0) return null;
	const first = items[0];
	const last = items[items.length - 1];
	const index = current === null ? -1 : items.indexOf(current);
	if (index === -1) return backwards ? last : first;
	if (backwards && index === 0) return last;
	if (!backwards && index === items.length - 1) return first;
	return null;
}

/** The controls inside `root` that Tab reaches: shown, and not taken out with tabindex=-1. */
export function focusables(root: HTMLElement): HTMLElement[] {
	return Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(
		(el) => el.tabIndex >= 0 && el.getClientRects().length > 0
	);
}

export function overlay(options: OverlayOptions = {}) {
	return (node: HTMLElement) => {
		const previous = document.activeElement;
		const trap = options.trap !== false;

		let frame = 0;
		if (options.autofocus !== false) {
			frame = requestAnimationFrame(() => {
				if (node.contains(document.activeElement)) return;
				const target = focusables(node)[0] ?? node;
				if (target === node && !node.hasAttribute('tabindex')) node.tabIndex = -1;
				target.focus({ preventScroll: true });
			});
		}

		const onKeydown = (e: KeyboardEvent) => {
			if (e.key === 'Escape' && options.onEscape && !e.defaultPrevented) {
				e.preventDefault();
				e.stopPropagation();
				options.onEscape();
				return;
			}
			if (e.key === 'Tab' && trap) {
				const active =
					document.activeElement instanceof HTMLElement ? document.activeElement : null;
				const next = wrapFocus(focusables(node), active, e.shiftKey);
				if (next) {
					e.preventDefault();
					next.focus();
				}
			}
		};
		node.addEventListener('keydown', onKeydown);

		return () => {
			cancelAnimationFrame(frame);
			node.removeEventListener('keydown', onKeydown);
			if (
				options.restoreFocus !== false &&
				previous instanceof HTMLElement &&
				previous.isConnected &&
				// Only when focus is still ours to give back: inside the closing overlay, or nowhere
				(node.contains(document.activeElement) || document.activeElement === document.body)
			) {
				previous.focus({ preventScroll: true });
			}
		};
	};
}
