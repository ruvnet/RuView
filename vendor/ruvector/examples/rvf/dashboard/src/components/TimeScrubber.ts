type ChangeCallback = (epoch: number) => void;

export class TimeScrubber {
  private root: HTMLElement;
  private slider: HTMLInputElement;
  private display: HTMLElement;
  private changeCallback: ChangeCallback | null = null;

  constructor(container: HTMLElement) {
    this.root = document.createElement('div');
    this.root.className = 'time-scrubber';

    const label = document.createElement('span');
    label.className = 'time-scrubber-title';
    label.textContent = 'Epoch';
    this.root.appendChild(label);

    this.slider = document.createElement('input');
    this.slider.type = 'range';
    this.slider.className = 'time-scrubber-range';
    this.slider.min = '0';
    this.slider.max = '100';
    this.slider.value = '0';
    this.slider.addEventListener('input', () => this.handleChange());
    this.root.appendChild(this.slider);

    this.display = document.createElement('span');
    this.display.className = 'time-scrubber-label';
    this.display.textContent = 'E0';
    this.root.appendChild(this.display);

    container.appendChild(this.root);
  }

  setRange(min: number, max: number): void {
    this.slider.min = String(min);
    this.slider.max = String(max);
    const val = Number(this.slider.value);
    if (val < min) this.slider.value = String(min);
    if (val > max) this.slider.value = String(max);
    this.updateDisplay();
  }

  setValue(epoch: number): void {
    this.slider.value = String(epoch);
    this.updateDisplay();
  }

  onChange(callback: ChangeCallback): void {
    this.changeCallback = callback;
  }

  private handleChange(): void {
    this.updateDisplay();
    this.changeCallback?.(Number(this.slider.value));
  }

  private updateDisplay(): void {
    this.display.textContent = `E${this.slider.value}`;
  }

  destroy(): void {
    this.root.remove();
  }
}
