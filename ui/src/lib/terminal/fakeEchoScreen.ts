/**
 * Test helper: a tiny terminal model behind the `EchoScreen` interface, so the predictive echo
 * can be driven by "the shell printed this" without xterm. It understands printable text,
 * CR, LF, BS and `ESC [ K`; everything else a real terminal does is out of scope.
 */

import type { EchoCell, EchoScreen } from './localEcho.js';

export class FakeEchoScreen implements EchoScreen {
	cols: number;
	lines: string[][] = [[]];
	cur = { line: 0, col: 0 };
	alternate = false;
	/** Every set of predictions handed to `show`, newest last */
	shown: EchoCell[][] = [];

	constructor(cols = 80) {
		this.cols = cols;
	}

	cursor() {
		return this.alternate ? null : { ...this.cur };
	}

	cell(line: number, col: number): string | null {
		const row = this.lines[line];
		if (!row) return null;
		return row[col] ?? ' ';
	}

	textBeforeCursor(): string {
		const row = this.lines[this.cur.line] ?? [];
		let text = '';
		for (let c = 0; c < this.cur.col; c++) text += row[c] ?? ' ';
		return text;
	}

	show(cells: EchoCell[]) {
		this.shown.push(cells.map((c) => ({ ...c })));
	}

	/** What is drawn provisionally right now, as text */
	get drawn(): string {
		return (this.shown[this.shown.length - 1] ?? []).map((c) => c.char).join('');
	}

	/** The shell printed this */
	print(text: string) {
		for (let i = 0; i < text.length; i++) {
			const ch = text[i];
			if (ch === '\r') this.cur.col = 0;
			else if (ch === '\n') {
				this.cur.line++;
				if (!this.lines[this.cur.line]) this.lines[this.cur.line] = [];
			} else if (ch === '\b') this.cur.col = Math.max(0, this.cur.col - 1);
			else if (ch === '\x1b' && text.startsWith('[K', i + 1)) {
				this.lines[this.cur.line].length = this.cur.col;
				i += 2;
			} else {
				const row = this.lines[this.cur.line];
				while (row.length < this.cur.col) row.push(' ');
				row[this.cur.col] = ch;
				this.cur.col = Math.min(this.cols - 1, this.cur.col + 1);
			}
		}
	}

	/** The text of an absolute line, trailing blanks removed */
	text(line = this.cur.line): string {
		return (this.lines[line] ?? []).join('').trimEnd();
	}
}
