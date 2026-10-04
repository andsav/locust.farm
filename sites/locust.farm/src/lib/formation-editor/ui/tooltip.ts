// Tooltips for the editor's icon controls. One shared element is placed under
// the control, or above it near the bottom of the window, so panels that clip
// their contents never cut a tooltip off. Mouse hover shows it after a short
// wait; keyboard focus shows it at once. A description is also given to screen
// readers as the control's aria-description.

export type TipText = string | { label: string; description?: string };

let shared: HTMLDivElement | null = null;
let owner: Element | null = null;
let timer: ReturnType<typeof setTimeout> | undefined;

function tipElement(): HTMLDivElement {
	if (!shared || !shared.isConnected) {
		shared = document.createElement('div');
		shared.className = 'bp-tip';
		shared.setAttribute('role', 'tooltip');
		shared.hidden = true;
		document.body.append(shared);
	}
	return shared;
}

function render(text: TipText) {
	const element = tipElement();
	const { label, description } = typeof text === 'string' ? { label: text } : text;
	const title = document.createElement('span');
	title.className = 'bp-tip-label';
	title.textContent = label;
	element.replaceChildren(title);
	if (description) {
		const more = document.createElement('span');
		more.className = 'bp-tip-description';
		more.textContent = description;
		element.append(more);
	}
}

function place(node: Element) {
	const element = tipElement();
	element.hidden = false;
	const anchor = node.getBoundingClientRect();
	const size = element.getBoundingClientRect();
	let top = anchor.bottom + 8;
	if (top + size.height > innerHeight - 8) top = anchor.top - size.height - 8;
	const left = Math.min(
		Math.max(8, anchor.left + anchor.width / 2 - size.width / 2),
		innerWidth - size.width - 8
	);
	element.style.top = `${Math.max(8, top)}px`;
	element.style.left = `${left}px`;
}

function hideTip() {
	clearTimeout(timer);
	owner = null;
	if (shared) shared.hidden = true;
	removeEventListener('keydown', onEscape, true);
}

function onEscape(event: KeyboardEvent) {
	if (event.key === 'Escape') hideTip();
}

export function tip(node: HTMLElement | SVGElement, text: TipText) {
	let current = text;

	function describe() {
		const description = typeof current === 'string' ? undefined : current.description;
		if (description) node.setAttribute('aria-description', description);
		else node.removeAttribute('aria-description');
	}

	function show(delay: number) {
		clearTimeout(timer);
		timer = setTimeout(() => {
			if (!node.isConnected) return;
			owner = node;
			render(current);
			place(node);
			addEventListener('keydown', onEscape, true);
		}, delay);
	}

	function hide() {
		if (owner === node || owner === null) hideTip();
	}

	const onEnter = (event: Event) => {
		if ((event as PointerEvent).pointerType === 'mouse') show(350);
	};
	const onFocus = (event: Event) => {
		if ((event.target as Element).matches(':focus-visible')) show(0);
	};

	node.addEventListener('pointerenter', onEnter);
	node.addEventListener('pointerleave', hide);
	node.addEventListener('pointerdown', hide);
	node.addEventListener('focusin', onFocus);
	node.addEventListener('focusout', hide);
	describe();

	return {
		update(next: TipText) {
			current = next;
			describe();
			if (owner === node) {
				render(current);
				place(node);
			}
		},
		destroy() {
			hide();
			node.removeEventListener('pointerenter', onEnter);
			node.removeEventListener('pointerleave', hide);
			node.removeEventListener('pointerdown', hide);
			node.removeEventListener('focusin', onFocus);
			node.removeEventListener('focusout', hide);
		}
	};
}
