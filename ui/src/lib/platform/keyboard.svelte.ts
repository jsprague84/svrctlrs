/**
 * Keyboard visibility detection (web fallback).
 * Tauri mobile will override this with native keyboard events in Phase 4.
 *
 * Heuristic: if the visual viewport height shrinks by >150px from the
 * initial "keyboard hidden" height, assume the keyboard is visible.
 */

let keyboardVisible = $state(false);
let initialHeight = 0;
let initialized = false;

const HEIGHT_THRESHOLD = 150;

export function isKeyboardVisible(): boolean {
	return keyboardVisible;
}

/** Initialize keyboard detection — call from onMount */
export function initKeyboardDetection() {
	if (initialized || typeof window === 'undefined') return;
	initialized = true;

	initialHeight = window.visualViewport?.height ?? window.innerHeight;

	if (window.visualViewport) {
		window.visualViewport.addEventListener('resize', handleResize);
	}

	// iOS Safari fallback: focusin/focusout on input elements
	document.addEventListener('focusin', handleFocusIn);
	document.addEventListener('focusout', handleFocusOut);
}

/** Clean up listeners */
export function destroyKeyboardDetection() {
	if (!initialized) return;
	initialized = false;

	if (window.visualViewport) {
		window.visualViewport.removeEventListener('resize', handleResize);
	}
	document.removeEventListener('focusin', handleFocusIn);
	document.removeEventListener('focusout', handleFocusOut);
}

function handleResize() {
	const currentHeight = window.visualViewport?.height ?? window.innerHeight;
	keyboardVisible = initialHeight - currentHeight > HEIGHT_THRESHOLD;
}

function handleFocusIn(e: FocusEvent) {
	const target = e.target as HTMLElement;
	if (target.tagName === 'INPUT' || target.tagName === 'TEXTAREA' || target.isContentEditable) {
		// Delay to let viewport resize happen first
		setTimeout(() => {
			const currentHeight = window.visualViewport?.height ?? window.innerHeight;
			if (initialHeight - currentHeight > HEIGHT_THRESHOLD) {
				keyboardVisible = true;
			}
		}, 300);
	}
}

function handleFocusOut() {
	setTimeout(() => {
		const currentHeight = window.visualViewport?.height ?? window.innerHeight;
		if (initialHeight - currentHeight <= HEIGHT_THRESHOLD) {
			keyboardVisible = false;
		}
	}, 200);
}
