import { scaleLinear } from 'd3-scale';
import { select } from 'd3-selection';
import { line } from 'd3-shape';
import { axisBottom, axisLeft } from 'd3-axis';

export interface LightCurvePoint {
  time: number;
  flux: number;
}

export interface TransitRegion {
  start: number;
  end: number;
}

export class LightCurveChart {
  private container: HTMLElement;
  private svg: SVGSVGElement | null = null;
  private wrapper: HTMLElement | null = null;
  private tooltip: HTMLElement | null = null;
  private crosshairLine: SVGLineElement | null = null;
  private crosshairDot: SVGCircleElement | null = null;
  private margin = { top: 28, right: 16, bottom: 40, left: 52 };
  private lastData: LightCurvePoint[] = [];
  private lastTransits: TransitRegion[] = [];

  constructor(container: HTMLElement) {
    this.container = container;
    this.createSvg();
  }

  private createSvg(): void {
    this.wrapper = document.createElement('div');
    this.wrapper.className = 'chart-container';
    this.wrapper.style.position = 'relative';
    this.container.appendChild(this.wrapper);

    // Title
    const title = document.createElement('h3');
    title.textContent = 'Light Curve';
    this.wrapper.appendChild(title);

    this.svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    this.svg.style.cursor = 'crosshair';
    this.wrapper.appendChild(this.svg);

    // Tooltip element
    this.tooltip = document.createElement('div');
    this.tooltip.style.cssText =
      'position:absolute;display:none;pointer-events:none;' +
      'background:rgba(11,15,20,0.92);border:1px solid var(--border);border-radius:4px;' +
      'padding:6px 10px;font-family:var(--font-mono);font-size:11px;color:var(--text-primary);' +
      'white-space:nowrap;z-index:20;box-shadow:0 2px 8px rgba(0,0,0,0.4)';
    this.wrapper.appendChild(this.tooltip);

    // Mouse tracking
    this.svg.addEventListener('mousemove', this.onMouseMove);
    this.svg.addEventListener('mouseleave', this.onMouseLeave);
  }

  private onMouseMove = (e: MouseEvent): void => {
    if (!this.svg || !this.tooltip || !this.wrapper || this.lastData.length === 0) return;

    const rect = this.svg.getBoundingClientRect();
    const svgW = rect.width;
    const svgH = rect.height;
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;

    const m = this.margin;
    const innerW = svgW - m.left - m.right;
    const innerH = svgH - m.top - m.bottom;

    const localX = mouseX - m.left;
    if (localX < 0 || localX > innerW) {
      this.onMouseLeave();
      return;
    }

    // Map pixel to time
    const xExtent = [this.lastData[0].time, this.lastData[this.lastData.length - 1].time];
    const tFrac = localX / innerW;
    const tVal = xExtent[0] + tFrac * (xExtent[1] - xExtent[0]);

    // Find nearest point via binary search (sorted by time)
    let lo = 0, hi = this.lastData.length - 1;
    while (lo < hi - 1) {
      const mid = (lo + hi) >> 1;
      if (this.lastData[mid].time < tVal) lo = mid; else hi = mid;
    }
    const nearest = Math.abs(this.lastData[lo].time - tVal) < Math.abs(this.lastData[hi].time - tVal)
      ? this.lastData[lo] : this.lastData[hi];

    // Map flux to pixel Y (use reduce to avoid stack overflow)
    let yMin = this.lastData[0].flux, yMax = this.lastData[0].flux;
    for (let i = 1; i < this.lastData.length; i++) {
      if (this.lastData[i].flux < yMin) yMin = this.lastData[i].flux;
      if (this.lastData[i].flux > yMax) yMax = this.lastData[i].flux;
    }
    const yPad = (yMax - yMin) * 0.1 || 0.001;
    const yFrac = (nearest.flux - (yMin - yPad)) / ((yMax + yPad) - (yMin - yPad));
    const pixelY = m.top + innerH * (1 - yFrac);
    const pixelX = m.left + (nearest.time - xExtent[0]) / (xExtent[1] - xExtent[0]) * innerW;

    // In transit?
    const inTransit = this.lastTransits.some(t => nearest.time >= t.start && nearest.time <= t.end);

    // Update crosshair
    if (this.crosshairLine) {
      this.crosshairLine.setAttribute('x1', String(pixelX));
      this.crosshairLine.setAttribute('x2', String(pixelX));
      this.crosshairLine.setAttribute('y1', String(m.top));
      this.crosshairLine.setAttribute('y2', String(m.top + innerH));
      this.crosshairLine.style.display = '';
    }
    if (this.crosshairDot) {
      this.crosshairDot.setAttribute('cx', String(pixelX));
      this.crosshairDot.setAttribute('cy', String(pixelY));
      this.crosshairDot.style.display = '';
    }

    // Tooltip
    const transitTag = inTransit ? '<span style="color:#FF4D4D;font-weight:600"> TRANSIT</span>' : '';
    this.tooltip.innerHTML =
      `<div>Time: <strong>${nearest.time.toFixed(2)} d</strong></div>` +
      `<div>Flux: <strong>${nearest.flux.toFixed(5)}</strong>${transitTag}</div>`;
    this.tooltip.style.display = 'block';

    // Position tooltip
    const tipX = mouseX + 14;
    const tipY = mouseY - 10;
    this.tooltip.style.left = `${tipX}px`;
    this.tooltip.style.top = `${tipY}px`;
  };

  private onMouseLeave = (): void => {
    if (this.tooltip) this.tooltip.style.display = 'none';
    if (this.crosshairLine) this.crosshairLine.style.display = 'none';
    if (this.crosshairDot) this.crosshairDot.style.display = 'none';
  };

  update(data: LightCurvePoint[], transits?: TransitRegion[]): void {
    if (!this.svg || !this.wrapper || data.length === 0) return;

    this.lastData = data;
    this.lastTransits = transits ?? [];

    const rect = this.wrapper.getBoundingClientRect();
    const width = rect.width || 400;
    const height = rect.height || 200;

    this.svg.setAttribute('viewBox', `0 0 ${width} ${height}`);
    this.svg.setAttribute('width', String(width));
    this.svg.setAttribute('height', String(height));

    const m = this.margin;
    const innerW = width - m.left - m.right;
    const innerH = height - m.top - m.bottom;

    // Use reduce instead of spread to avoid stack overflow with large datasets
    let xMin = data[0].time, xMax = data[0].time, yMin = data[0].flux, yMax = data[0].flux;
    for (let i = 1; i < data.length; i++) {
      if (data[i].time < xMin) xMin = data[i].time;
      if (data[i].time > xMax) xMax = data[i].time;
      if (data[i].flux < yMin) yMin = data[i].flux;
      if (data[i].flux > yMax) yMax = data[i].flux;
    }
    const xExtent = [xMin, xMax];
    const yExtent = [yMin, yMax];
    const yPad = (yExtent[1] - yExtent[0]) * 0.1 || 0.001;

    const xScale = scaleLinear().domain(xExtent).range([0, innerW]);
    const yScale = scaleLinear().domain([yExtent[0] - yPad, yExtent[1] + yPad]).range([innerH, 0]);

    const sel = select(this.svg);
    sel.selectAll('*').remove();

    const g = sel.append('g').attr('transform', `translate(${m.left},${m.top})`);

    // Baseline reference at flux = 1.0
    if (yExtent[0] - yPad < 1.0 && yExtent[1] + yPad > 1.0) {
      g.append('line')
        .attr('x1', 0).attr('x2', innerW)
        .attr('y1', yScale(1.0)).attr('y2', yScale(1.0))
        .attr('stroke', '#484F58').attr('stroke-dasharray', '4,3').attr('stroke-width', 1);
      g.append('text')
        .attr('x', innerW - 4).attr('y', yScale(1.0) - 4)
        .attr('text-anchor', 'end').attr('fill', '#484F58').attr('font-size', '9')
        .text('baseline');
    }

    // Transit overlay rectangles with labels
    if (transits) {
      transits.forEach((t, i) => {
        const rx = xScale(t.start);
        const rw = Math.max(1, xScale(t.end) - xScale(t.start));

        g.append('rect')
          .attr('x', rx).attr('y', 0).attr('width', rw).attr('height', innerH)
          .attr('fill', 'rgba(255, 77, 77, 0.08)').attr('stroke', 'rgba(255, 77, 77, 0.2)')
          .attr('stroke-width', 1);

        // Transit label
        g.append('text')
          .attr('x', rx + rw / 2).attr('y', -4)
          .attr('text-anchor', 'middle').attr('fill', '#FF4D4D')
          .attr('font-size', '9').attr('font-weight', '600')
          .text(`T${i + 1}`);

        // Arrow pointing down
        g.append('line')
          .attr('x1', rx + rw / 2).attr('x2', rx + rw / 2)
          .attr('y1', 2).attr('y2', 14)
          .attr('stroke', '#FF4D4D').attr('stroke-width', 1)
          .attr('marker-end', 'url(#transit-arrow)');
      });
    }

    // Arrow marker definition
    sel.append('defs').append('marker')
      .attr('id', 'transit-arrow').attr('viewBox', '0 0 6 6')
      .attr('refX', 3).attr('refY', 3).attr('markerWidth', 5).attr('markerHeight', 5)
      .attr('orient', 'auto')
      .append('path').attr('d', 'M0,0 L6,3 L0,6 Z').attr('fill', '#FF4D4D');

    // Axes
    g.append('g')
      .attr('class', 'axis')
      .attr('transform', `translate(0,${innerH})`)
      .call(axisBottom(xScale).ticks(6));

    g.append('g').attr('class', 'axis').call(axisLeft(yScale).ticks(5));

    // Axis labels
    g.append('text')
      .attr('x', innerW / 2).attr('y', innerH + 32)
      .attr('text-anchor', 'middle').attr('fill', '#8B949E').attr('font-size', '10')
      .text('Time (days)');

    g.append('text')
      .attr('transform', `rotate(-90)`)
      .attr('x', -innerH / 2).attr('y', -38)
      .attr('text-anchor', 'middle').attr('fill', '#8B949E').attr('font-size', '10')
      .text('Relative Flux');

    // Data line
    const lineFn = line<LightCurvePoint>()
      .x(d => xScale(d.time))
      .y(d => yScale(d.flux));

    g.append('path')
      .datum(data)
      .attr('class', 'chart-line')
      .attr('d', lineFn);

    // Crosshair elements (hidden by default)
    this.crosshairLine = sel.append('line')
      .attr('stroke', 'rgba(0,229,255,0.4)').attr('stroke-width', 1)
      .attr('stroke-dasharray', '3,2').style('display', 'none')
      .node() as SVGLineElement;

    this.crosshairDot = sel.append('circle')
      .attr('r', 4).attr('fill', '#00E5FF').attr('stroke', '#0B0F14').attr('stroke-width', 2)
      .style('display', 'none')
      .node() as SVGCircleElement;
  }

  destroy(): void {
    if (this.svg) {
      this.svg.removeEventListener('mousemove', this.onMouseMove);
      this.svg.removeEventListener('mouseleave', this.onMouseLeave);
    }
    if (this.wrapper) this.wrapper.remove();
    this.svg = null;
    this.wrapper = null;
    this.tooltip = null;
    this.crosshairLine = null;
    this.crosshairDot = null;
  }
}
