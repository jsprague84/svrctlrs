let mobile = $state(false);

if (typeof window !== 'undefined') {
	const mql = window.matchMedia('(max-width: 767px)');
	mobile = mql.matches;
	mql.addEventListener('change', (e) => {
		mobile = e.matches;
	});
}

export function isMobile(): boolean {
	return mobile;
}
