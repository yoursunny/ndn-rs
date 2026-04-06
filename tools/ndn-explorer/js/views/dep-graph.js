import { LAYER_COLORS } from '../app.js';

export class DepGraph {
  constructor(container, app) {
    this.container = container;
    this.app = app;
    this.nodes = {};
    this.rendered = false;
    this.hoveredNode = null;
  }

  onShow() {
    if (!this.rendered) {
      this.container.innerHTML = `
        <h1 style="margin-bottom:1rem">Dependency Graph</h1>
        <div class="graph-container">
          <canvas id="graph-canvas"></canvas>
          <div class="graph-legend" id="graph-legend"></div>
          <div class="graph-tooltip" id="graph-tooltip">
            <div class="tt-name"></div>
            <div class="tt-desc"></div>
          </div>
        </div>`;
      this.rendered = true;
    }
    // Re-render on every show to handle resizes
    requestAnimationFrame(() => this._draw());
  }

  _draw() {
    const canvas = this.container.querySelector('#graph-canvas');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;

    const parent = canvas.parentElement;
    const W = parent.clientWidth;
    const H = 650;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    canvas.style.width = `${W}px`;
    canvas.style.height = `${H}px`;
    ctx.scale(dpr, dpr);

    // Filter out examples for cleaner graph
    const crates = this.app.data.crates.filter(c => c.layer !== 'examples');

    // Group by layer
    const layerGroups = {};
    crates.forEach(c => {
      if (!layerGroups[c.layer_num]) layerGroups[c.layer_num] = [];
      layerGroups[c.layer_num].push(c);
    });
    const layerNums = Object.keys(layerGroups).map(Number).sort((a, b) => b - a);

    // Position nodes
    this.nodes = {};
    const padding = 60;
    const usableW = W - padding * 2;
    const yStep = (H - padding * 2) / (layerNums.length - 1 || 1);

    layerNums.forEach((ln, yi) => {
      const group = layerGroups[ln];
      const xStep = usableW / (group.length + 1);
      group.forEach((c, xi) => {
        this.nodes[c.name] = {
          x: padding + xStep * (xi + 1),
          y: padding + yStep * yi,
          r: 7,
          color: LAYER_COLORS[c.layer] || '#8b949e',
          crate: c,
        };
      });
    });

    // Clear
    ctx.clearRect(0, 0, W, H);

    // Draw edges with curved lines
    crates.forEach(c => {
      const from = this.nodes[c.name];
      if (!from) return;
      c.workspace_deps.forEach(dep => {
        const to = this.nodes[dep];
        if (!to) return;

        const isHighlighted = this.hoveredNode &&
          (this.hoveredNode === c.name || this.hoveredNode === dep);

        ctx.beginPath();
        ctx.moveTo(from.x, from.y);
        // Gentle curve
        const cpx = (from.x + to.x) / 2;
        const cpy = (from.y + to.y) / 2 + (from.x - to.x) * 0.1;
        ctx.quadraticCurveTo(cpx, cpy, to.x, to.y);
        ctx.strokeStyle = isHighlighted ? '#58a6ff44' : '#30363d66';
        ctx.lineWidth = isHighlighted ? 2 : 1;
        ctx.stroke();
      });
    });

    // Draw nodes
    Object.values(this.nodes).forEach(n => {
      const isHovered = this.hoveredNode === n.crate.name;
      const isConnected = this.hoveredNode && (
        n.crate.workspace_deps.includes(this.hoveredNode) ||
        this.app.getCrate(this.hoveredNode)?.workspace_deps.includes(n.crate.name)
      );
      const dimmed = this.hoveredNode && !isHovered && !isConnected;

      ctx.beginPath();
      const r = isHovered ? n.r + 2 : n.r;
      ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
      ctx.fillStyle = dimmed ? n.color + '44' : n.color;
      ctx.fill();

      if (isHovered) {
        ctx.strokeStyle = '#ffffff44';
        ctx.lineWidth = 2;
        ctx.stroke();
      }

      ctx.font = `${isHovered ? '600 ' : ''}11px -apple-system, sans-serif`;
      ctx.fillStyle = dimmed ? '#8b949e66' : '#e6edf3';
      ctx.textAlign = 'center';
      ctx.fillText(n.crate.name, n.x, n.y - r - 5);
    });

    // Draw layer labels on left
    layerNums.forEach((ln, yi) => {
      const layer = this.app.data.layers.find(l => l.num === ln);
      if (!layer) return;
      ctx.font = '10px -apple-system, sans-serif';
      ctx.fillStyle = '#8b949e';
      ctx.textAlign = 'left';
      ctx.fillText(layer.label, 8, padding + yStep * yi + 3);
    });

    // Build legend
    this._buildLegend();

    // Wire up interactions
    this._wireEvents(canvas);
  }

  _buildLegend() {
    const legend = this.container.querySelector('#graph-legend');
    if (!legend) return;
    const seen = new Set();
    const items = [];
    this.app.data.layers.forEach(layer => {
      const hasCrates = this.app.data.crates.some(c => c.layer === layer.id && c.layer !== 'examples');
      if (!hasCrates || seen.has(layer.id)) return;
      seen.add(layer.id);
      const color = LAYER_COLORS[layer.id] || '#8b949e';
      items.push(`<div class="legend-item"><span class="legend-dot" style="background:${color}"></span>${layer.label}</div>`);
    });
    legend.innerHTML = items.join('');
  }

  _wireEvents(canvas) {
    const tooltip = this.container.querySelector('#graph-tooltip');

    canvas.onmousemove = (e) => {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      let found = null;

      for (const n of Object.values(this.nodes)) {
        if (Math.hypot(mx - n.x, my - n.y) < 14) {
          found = n;
          break;
        }
      }

      const newHovered = found ? found.crate.name : null;
      if (newHovered !== this.hoveredNode) {
        this.hoveredNode = newHovered;
        this._draw();
      }

      if (found && tooltip) {
        tooltip.querySelector('.tt-name').textContent = found.crate.name;
        tooltip.querySelector('.tt-desc').textContent = found.crate.description;
        tooltip.style.left = `${mx + 16}px`;
        tooltip.style.top = `${my - 10}px`;
        tooltip.classList.add('visible');
      } else if (tooltip) {
        tooltip.classList.remove('visible');
      }
    };

    canvas.onmouseleave = () => {
      if (this.hoveredNode) {
        this.hoveredNode = null;
        this._draw();
      }
      if (tooltip) tooltip.classList.remove('visible');
    };

    canvas.onclick = (e) => {
      const rect = canvas.getBoundingClientRect();
      const mx = e.clientX - rect.left;
      const my = e.clientY - rect.top;
      for (const n of Object.values(this.nodes)) {
        if (Math.hypot(mx - n.x, my - n.y) < 14) {
          this.app.showCrate(n.crate.name);
          return;
        }
      }
    };
  }
}
