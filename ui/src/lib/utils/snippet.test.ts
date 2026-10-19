import { describe, expect, it } from 'vitest';
import { fillSnippet, hasVariables, promptedVariables, serverVariables } from './snippet.js';

const web = serverVariables({ hostname: '10.0.0.5', port: 2222, username: 'deploy', name: 'web' });

describe('quick-command variables', () => {
	it("fills the built-ins from the tab's server", () => {
		const cmd = 'ssh-keyscan -p {{port}} {{host}} && echo {{user}}@{{name}}';
		expect(promptedVariables(cmd, web)).toEqual([]);
		expect(fillSnippet(cmd, web)).toBe('ssh-keyscan -p 2222 10.0.0.5 && echo deploy@web');
	});

	it('asks for the rest once each, in order, with a default when the snippet gives one', () => {
		const cmd = 'journalctl -u {{unit}} -n {{lines:200}} --since "{{since}}" | grep {{unit}}';
		expect(promptedVariables(cmd, web)).toEqual([
			{ name: 'unit', default: '' },
			{ name: 'lines', default: '200' },
			{ name: 'since', default: '' }
		]);
		const filled = fillSnippet(cmd, { ...web, unit: 'nginx', lines: '50', since: '1 hour ago' });
		expect(filled).toBe('journalctl -u nginx -n 50 --since "1 hour ago" | grep nginx');
	});

	it('takes a default from any occurrence and uses it when no answer is given', () => {
		expect(promptedVariables('cd {{dir}}; ls {{dir:/var/log}}')).toEqual([
			{ name: 'dir', default: '/var/log' }
		]);
		expect(fillSnippet('tail -n {{n:20}} x', {})).toBe('tail -n 20 x');
	});

	it('allows spaces inside the braces', () => {
		expect(fillSnippet('ping {{ host }} -c {{ count : 3 }}', web)).toBe('ping 10.0.0.5 -c 3');
	});

	it('leaves Go templates and other braces alone', () => {
		for (const cmd of [
			"docker ps --format '{{.Names}}\\t{{.Status}}'",
			"docker inspect -f '{{json .Config}}' app",
			"kubectl get pods -o go-template='{{range .items}}{{.metadata.name}}{{end}}'",
			"helm template x --set a='{{if .A}}{{.A}}{{else}}none{{end}}'",
			'echo {{1abc}} {single} {{}}',
			'awk "{print $1}"'
		]) {
			expect(hasVariables(cmd)).toBe(false);
			expect(fillSnippet(cmd, web)).toBe(cmd);
		}
	});

	it('fills variables beside a Go template in the same snippet', () => {
		const cmd = "docker ps --filter name={{app}} --format '{{.Names}} {{end}}'";
		expect(promptedVariables(cmd, web)).toEqual([{ name: 'app', default: '' }]);
		expect(fillSnippet(cmd, { app: 'api' })).toBe(
			"docker ps --filter name=api --format '{{.Names}} {{end}}'"
		);
	});

	it('asks for a built-in the server cannot provide', () => {
		const noUser = serverVariables({ hostname: 'h', port: 22, username: null, name: 'n' });
		expect(promptedVariables('sudo -u {{user}} id', noUser)).toEqual([
			{ name: 'user', default: '' }
		]);
	});

	it('inserts answers as given, without quoting', () => {
		expect(fillSnippet('ls {{glob}}', { glob: '*.log | wc -l' })).toBe('ls *.log | wc -l');
	});
});
