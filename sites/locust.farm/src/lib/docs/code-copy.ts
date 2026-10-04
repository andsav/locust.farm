import { copyText } from '../onboarding/clipboard.ts';

/** Enhance rendered, reviewed code blocks. Plain pre/code remains readable without JS. */
export function codeCopy(element: HTMLElement, html: string) {
	void html;
	let cleanups: (() => void)[] = [];
	function enhance() {
		cleanups.forEach((cleanup) => cleanup());
		cleanups = [];
		element.querySelectorAll('pre').forEach((pre, index) => {
			const controls = document.createElement('div');
			controls.className = 'code-controls';
			const button = document.createElement('button');
			button.type = 'button';
			button.textContent = 'Copy code';
			button.setAttribute('aria-label', `Copy code block ${index + 1}`);
			const status = document.createElement('span');
			status.setAttribute('role', 'status');
			status.setAttribute('aria-live', 'polite');
			controls.append(button, status);
			pre.after(controls);
			const copy = async () => {
				status.textContent = '';
				const result = await copyText(pre.textContent ?? '', navigator.clipboard);
				status.textContent =
					result === 'copied'
						? 'Code copied.'
						: 'Copy failed. Code selected: press Ctrl+C or Cmd+C to copy it.';
				if (result !== 'copied') {
					const range = document.createRange();
					range.selectNodeContents(pre);
					window.getSelection()?.removeAllRanges();
					window.getSelection()?.addRange(range);
				}
			};
			button.addEventListener('click', copy);
			cleanups.push(() => {
				button.removeEventListener('click', copy);
				controls.remove();
			});
		});
	}
	enhance();
	return {
		update(html: string) {
			void html;
			enhance();
		},
		destroy() {
			cleanups.forEach((cleanup) => cleanup());
		}
	};
}
