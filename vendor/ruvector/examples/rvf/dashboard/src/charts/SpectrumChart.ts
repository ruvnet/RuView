import { scaleLinear } from 'd3-scale';
import { select } from 'd3-selection';
import { line } from 'd3-shape';
import { axisBottom, axisLeft } from 'd3-axis';

export interface SpectrumPoint {
  wavelength: number;
  flux: number;
}

export interface SpectrumBand {
  name: string;
  start: number;
  end: number;
  color: string;
}

export class SpectrumChart {
  private container: HTMLElement;
  private svg: SVGSVGElement | null = null;
  private wrapper: HTMLElement | null = null;
  private margin = { top: 16, right: 16, bottom: 32, left: 48 };

  constructor(container: HTMLElement) {
    this.container = container;
    this.createSvg();
  }

  private createSvg(): void {
    this.wrapper = document.createElement('div');
    this.wrapper.className = 'chart-container';
    this.container.appendChild(this.wrapper);

    this.svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    this.wrapper.appendChild(this.svg);
  }

  update(data: SpectrumPoint[], bands?: SpectrumBand[]): void {
    if (!this.svg || !this.wrapper || data.length === 0) return;

    const rect = this.wrapper.getBoundingClientRect();
    const width = rect.width || 400;
    const height = rect.height || 200;

    this.svg.setAttribute('viewBox', `0 0 ${width} ${height}`);

    const m = this.margin;
    const innerW = width - m.left - m.right;
    const innerH = height - m.top - m.bottom;

    // Use loop to avoid stack overflow with large datasets
    let xMin = data[0].wavelength, xMax = data[0].wavelength;
    let yMin = data[0].flux, yMax = data[0].flux;
    for (let i = 1; i < data.length; i++) {
      if (data[i].wavelength < xMin) xMin = data[i].wavelength;
      if (data[i].wavelength > xMax) xMax = data[i].wavelength;
      if (data[i].flux < yMin) yMin = data[i].flux;
      if (data[i].flux > yMax) yMax = data[i].flux;
    }
    const xExtent = [xMin, xMax];
    const yExtent = [yMin, yMax];
    const yPad = (yExtent[1] - yExtent[0]) * 0.1 || 0.001;

    const xScale = scaleLinear().domain(xExtent).range([0, innerW]);
    const yScale = scaleLinear()
      .domain([yExtent[0] - yPad, yExtent[1] + yPad])
      .range([innerH, 0]);

    const sel = select(this.svg);
    sel.selectAll('*').remove();

    const g = sel
      .append('g')
      .attr('transform', `translate(${m.left},${m.top})`);

    // Molecule absorption bands
    if (bands) {
      for (const b of bands) {
        g.append('rect')
          .attr('class', 'band-rect')
          .attr('x', xScale(b.start))
          .attr('y', 0)
          .attr('width', Math.max(1, xScale(b.end) - xScale(b.start)))
          .attr('height', innerH)
          .attr('fill', b.color);

        g.append('text')
          .attr('x', xScale((b.start + b.end) / 2))
          .attr('y', 10)
          .attr('text-anchor', 'middle')
          .attr('fill', b.color)
          .attr('font-size', '9px')
          .text(b.name);
      }
    }

    // Axes
    g.append('g')
      .attr('class', 'axis')
      .attr('transform', `translate(0,${innerH})`)
      .call(axisBottom(xScale).ticks(6));

    g.append('g').attr('class', 'axis').call(axisLeft(yScale).ticks(5));

    // Spectrum line
    const lineFn = line<SpectrumPoint>()
      .x((d) => xScale(d.wavelength))
      .y((d) => yScale(d.flux));

    g.append('path')
      .datum(data)
      .attr('class', 'chart-line')
      .attr('d', lineFn)
      .attr('stroke', '#2ECC71');
  }

  destroy(): void {
    if (this.wrapper) {
      this.wrapper.remove();
    }
    this.svg = null;
    this.wrapper = null;
  }
}
