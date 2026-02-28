import { scaleLinear } from 'd3-scale';
import { select } from 'd3-selection';

export interface RadarScore {
  label: string;
  value: number;
}

export class RadarChart {
  private container: HTMLElement;
  private svg: SVGSVGElement | null = null;
  private wrapper: HTMLElement | null = null;

  constructor(container: HTMLElement) {
    this.container = container;
    this.createSvg();
  }

  private createSvg(): void {
    this.wrapper = document.createElement('div');
    this.wrapper.className = 'chart-container';
    this.container.appendChild(this.wrapper);

    // Title
    const title = document.createElement('h3');
    title.textContent = 'Detection Quality';
    this.wrapper.appendChild(title);

    this.svg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
    this.svg.setAttribute('preserveAspectRatio', 'xMidYMid meet');
    this.wrapper.appendChild(this.svg);
  }

  update(scores: RadarScore[]): void {
    if (!this.svg || !this.wrapper || scores.length === 0) return;

    const rect = this.wrapper.getBoundingClientRect();
    const size = Math.min(rect.width || 200, rect.height || 200);
    const cx = size / 2;
    const cy = size / 2;
    const radius = size / 2 - 40;

    this.svg.setAttribute('viewBox', `0 0 ${size} ${size}`);

    const sel = select(this.svg);
    sel.selectAll('*').remove();

    const g = sel.append('g').attr('transform', `translate(${cx},${cy})`);

    const n = scores.length;
    const angleSlice = (Math.PI * 2) / n;

    const rScale = scaleLinear().domain([0, 1]).range([0, radius]);

    // Grid polygons with level labels
    const levels = 4;
    for (let lev = 1; lev <= levels; lev++) {
      const r = (radius / levels) * lev;
      const pts: string[] = [];
      for (let j = 0; j < n; j++) {
        const angle = j * angleSlice - Math.PI / 2;
        pts.push(`${r * Math.cos(angle)},${r * Math.sin(angle)}`);
      }
      g.append('polygon')
        .attr('class', 'radar-grid')
        .attr('points', pts.join(' '));

      // Level value label on the first axis
      const labelAngle = -Math.PI / 2;
      const labelVal = (lev / levels);
      g.append('text')
        .attr('x', r * Math.cos(labelAngle) + 4)
        .attr('y', r * Math.sin(labelAngle) - 2)
        .attr('fill', '#484F58').attr('font-size', '8').attr('font-family', 'var(--font-mono)')
        .text(labelVal.toFixed(2));
    }

    // Axis lines
    for (let i = 0; i < n; i++) {
      const angle = i * angleSlice - Math.PI / 2;
      g.append('line')
        .attr('class', 'radar-grid')
        .attr('x1', 0).attr('y1', 0)
        .attr('x2', radius * Math.cos(angle))
        .attr('y2', radius * Math.sin(angle));
    }

    // Labels with values
    for (let i = 0; i < n; i++) {
      const angle = i * angleSlice - Math.PI / 2;
      const lx = (radius + 22) * Math.cos(angle);
      const ly = (radius + 22) * Math.sin(angle);

      // Label text
      g.append('text')
        .attr('class', 'radar-label')
        .attr('x', lx).attr('y', ly - 5)
        .attr('dy', '0.35em')
        .attr('font-size', '10')
        .text(scores[i].label);

      // Value below label
      const val = scores[i].value;
      const color = val > 0.7 ? '#2ECC71' : val > 0.4 ? '#FFB020' : '#FF4D4D';
      g.append('text')
        .attr('x', lx).attr('y', ly + 8)
        .attr('text-anchor', 'middle')
        .attr('fill', color)
        .attr('font-size', '10').attr('font-weight', '600')
        .attr('font-family', 'var(--font-mono)')
        .text(val.toFixed(2));
    }

    // Data polygon
    const polyPoints: string[] = [];
    for (let i = 0; i < n; i++) {
      const angle = i * angleSlice - Math.PI / 2;
      const r = rScale(Math.max(0, Math.min(1, scores[i].value)));
      polyPoints.push(`${r * Math.cos(angle)},${r * Math.sin(angle)}`);
    }

    g.append('polygon')
      .attr('class', 'radar-polygon')
      .attr('points', polyPoints.join(' '));

    // Data dots with value tooltips
    for (let i = 0; i < n; i++) {
      const angle = i * angleSlice - Math.PI / 2;
      const r = rScale(Math.max(0, Math.min(1, scores[i].value)));
      const cx = r * Math.cos(angle);
      const cy = r * Math.sin(angle);

      // Outer glow
      g.append('circle')
        .attr('cx', cx).attr('cy', cy).attr('r', 5)
        .attr('fill', 'rgba(0,229,255,0.15)').attr('stroke', 'none');

      // Dot
      g.append('circle')
        .attr('cx', cx).attr('cy', cy).attr('r', 3)
        .attr('fill', '#00E5FF');
    }
  }

  destroy(): void {
    if (this.wrapper) this.wrapper.remove();
    this.svg = null;
    this.wrapper = null;
  }
}
