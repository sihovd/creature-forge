/** Et valg i hurtigmenyen. `null` gir en skillelinje. */
export type MenuItem = { label: string; action: () => void; disabled?: boolean; hint?: string } | null;

/**
 * Hurtigmenyen som åpnes ved høyreklikk i arbeidsflaten. Den lukkes ved
 * klikk utenfor, Esc eller når et valg er gjort.
 */
export class ContextMenu {
  readonly el: HTMLDivElement;

  constructor(parent: HTMLElement) {
    this.el = document.createElement('div');
    this.el.className = 'context-menu';
    this.el.setAttribute('role', 'menu');
    this.el.hidden = true;
    parent.append(this.el);
    window.addEventListener('pointerdown', (e) => {
      if (!this.el.hidden && !this.el.contains(e.target as Node)) this.hide();
    }, { capture: true });
    window.addEventListener('keydown', (e) => e.key === 'Escape' && this.hide());
    window.addEventListener('blur', () => this.hide());
  }

  get open() {
    return !this.el.hidden;
  }

  show(clientX: number, clientY: number, title: string, items: MenuItem[]) {
    this.el.replaceChildren();
    const head = document.createElement('div');
    head.className = 'context-title';
    head.textContent = title;
    this.el.append(head);
    for (const item of items) {
      if (!item) {
        this.el.append(Object.assign(document.createElement('hr'), {}));
        continue;
      }
      const b = document.createElement('button');
      b.type = 'button';
      b.setAttribute('role', 'menuitem');
      b.textContent = item.label;
      if (item.hint) {
        const k = document.createElement('kbd');
        k.textContent = item.hint;
        b.append(k);
      }
      b.disabled = !!item.disabled;
      b.addEventListener('click', () => {
        this.hide();
        item.action();
      });
      this.el.append(b);
    }
    this.el.hidden = false;
    // Hold menyen innenfor vinduet.
    const r = this.el.getBoundingClientRect();
    const x = Math.min(clientX, window.innerWidth - r.width - 4);
    const y = Math.min(clientY, window.innerHeight - r.height - 4);
    this.el.style.left = `${Math.max(4, x)}px`;
    this.el.style.top = `${Math.max(4, y)}px`;
    (this.el.querySelector('button:not(:disabled)') as HTMLButtonElement | null)?.focus({ preventScroll: true });
  }

  hide() {
    this.el.hidden = true;
  }
}
