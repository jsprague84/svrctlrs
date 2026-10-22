import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import { colorTokens, contrastRatio, over, parseHex } from './contrast.js';
import { lightTheme } from '#lib/components/terminal/terminal-theme.js';

// Both themes' tokens, straight from the stylesheet the app ships
const css = readFileSync(new URL('../../app.css', import.meta.url), 'utf8');
const darkBlock = css.slice(css.indexOf('@theme {'), css.indexOf("[data-theme='light'] {"));
const lightStart = css.indexOf("[data-theme='light'] {");
const lightBlock = css.slice(lightStart, css.indexOf('}', lightStart));
const dark = colorTokens(darkBlock);
const light = { ...dark, ...colorTokens(lightBlock) };

/** Colours used for text, and the surfaces text sits on */
const TEXT = [
	'text-primary',
	'text-secondary',
	'text-muted',
	'accent',
	'success',
	'warning',
	'error',
	'info',
	'sidebar-muted'
];
const SURFACES = ['background', 'surface', 'surface-raised', 'sidebar'];
const AA = 4.5;

describe.each([
	['dark', dark],
	['light', light]
])('%s theme', (_name, tokens) => {
	it.each(TEXT)('%s reads at AA on every surface and on its own badge tint', (fg) => {
		const color = parseHex(tokens[fg]);
		for (const bg of SURFACES) {
			const ratio = contrastRatio(color, parseHex(tokens[bg]));
			expect(ratio, `${fg} on ${bg}`).toBeGreaterThanOrEqual(AA);
		}
		// Badge and toast: text-x on bg-x/10 over the surface
		const tint = over(color, parseHex(tokens.surface), 0.1);
		expect(contrastRatio(color, tint), `${fg} on its 10% tint`).toBeGreaterThanOrEqual(AA);
	});

	it.each([
		['primary-foreground', 'primary'],
		['destructive-foreground', 'destructive'],
		['secondary-foreground', 'secondary'],
		['accent-foreground', 'accent']
	])('%s on %s (buttons)', (fg, bg) => {
		expect(contrastRatio(parseHex(tokens[fg]), parseHex(tokens[bg]))).toBeGreaterThanOrEqual(AA);
	});

	it.each(['background', 'surface', 'surface-raised'])(
		'input outlines and the focus ring stand out on %s (3:1, WCAG 1.4.11)',
		(bg) => {
			for (const fg of ['border-strong', 'ring']) {
				const ratio = contrastRatio(parseHex(tokens[fg]), parseHex(tokens[bg]));
				expect(ratio, `${fg} on ${bg}`).toBeGreaterThanOrEqual(3);
			}
		}
	);
});

describe('light terminal theme', () => {
	// Dark keeps Tokyo Night as published (its bright black is a dim comment colour on purpose)
	const ansi = Object.entries(lightTheme).filter(
		([k]) => !/background|cursor|selection|black$/i.test(k) || k === 'brightBlack'
	);
	it.each(ansi)('%s reads at AA on the terminal background', (_name, color) => {
		const ratio = contrastRatio(parseHex(color as string), parseHex(lightTheme.background!));
		expect(ratio).toBeGreaterThanOrEqual(AA);
	});
});

describe('contrast arithmetic', () => {
	it('matches the WCAG reference values', () => {
		expect(contrastRatio(parseHex('#000000'), parseHex('#ffffff'))).toBeCloseTo(21, 5);
		expect(contrastRatio(parseHex('#777777'), parseHex('#ffffff'))).toBeCloseTo(4.48, 2);
	});
});
