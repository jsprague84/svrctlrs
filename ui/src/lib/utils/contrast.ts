/** WCAG 2 contrast arithmetic for the theme tokens (see theme-contrast.test.ts). */

export type Rgb = [number, number, number];

export function parseHex(hex: string): Rgb {
	const h = hex.replace('#', '');
	if (!/^[0-9a-f]{6}$/i.test(h)) throw new Error(`not a #rrggbb colour: ${hex}`);
	return [0, 2, 4].map((i) => parseInt(h.slice(i, i + 2), 16) / 255) as Rgb;
}

function relativeLuminance([r, g, b]: Rgb): number {
	const ch = (v: number) => (v <= 0.03928 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4);
	return 0.2126 * ch(r) + 0.7152 * ch(g) + 0.0722 * ch(b);
}

export function contrastRatio(a: Rgb, b: Rgb): number {
	const [hi, lo] = [relativeLuminance(a), relativeLuminance(b)].sort((x, y) => y - x);
	return (hi + 0.05) / (lo + 0.05);
}

/** `fg` at `alpha` over `bg`, the way `bg-warning/10` paints a badge. */
export function over(fg: Rgb, bg: Rgb, alpha: number): Rgb {
	return fg.map((v, i) => v * alpha + bg[i] * (1 - alpha)) as Rgb;
}

/** `--color-*: #hex` declarations of one CSS block, by token name without the prefix. */
export function colorTokens(block: string): Record<string, string> {
	const out: Record<string, string> = {};
	for (const m of block.matchAll(/--color-([a-z-]+):\s*(#[0-9a-fA-F]{6})\b/g)) out[m[1]] = m[2];
	return out;
}
