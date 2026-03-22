let mobile = $state(false);

if (typeof window !== 'undefined') {
	// Mobile: narrow viewport OR touch device with small height (landscape phone)
	const check = () => {
		const narrow = window.innerWidth < 768;
		const landscapePhone = 'ontouchstart' in window && window.innerHeight < 500;
		mobile = narrow || landscapePhone;
	};
	check();
	window.addEventListener('resize', check);
}

export function isMobile(): boolean {
	return mobile;
}
