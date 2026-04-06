import { LAYER_COLORS } from '../app.js';

export class CrateDetail {
  constructor(container, app) {
    this.container = container;
    this.app = app;
  }

  onShow(params) {
    if (!params || !params.name) return;
    this.render(params.name);
  }

  render(name) {
    const c = this.app.getCrate(name);
    if (!c) {
      this.container.innerHTML = `<p>Crate "${name}" not found.</p>`;
      return;
    }

    const layer = this.app.getLayer(c.layer);
    const color = LAYER_COLORS[c.layer] || '#8b949e';
    const rdeps = this.app.getReverseDeps(c.name);
    const featureEntries = Object.entries(c.features);

    this.container.innerHTML = `
      <button class="back-btn" id="detail-back">&larr; Back</button>

      <div class="detail-header">
        <h1 style="color:${color}">${c.name}</h1>
        <div class="desc">${c.description}</div>
        <div class="badges" style="margin-top:0.5rem">
          <span class="badge" style="color:${color};border-color:${color}">${layer ? layer.label : c.layer}</span>
          ${c.no_std ? '<span class="badge badge-green">no_std</span>' : ''}
          <span class="badge">${c.path}</span>
        </div>
      </div>

      <div class="detail-grid">
        <!-- Key Types -->
        <div class="detail-panel">
          <div class="panel-title">Key Types (${c.key_types.length})</div>
          ${c.key_types.length > 0
            ? `<ul class="type-list">${c.key_types.map(t => `<li>${t}</li>`).join('')}</ul>`
            : '<p style="color:var(--text2);font-size:0.85rem">No public types exported</p>'}
        </div>

        <!-- Dependencies -->
        <div class="detail-panel">
          <div class="panel-title">Depends On (${c.workspace_deps.length})</div>
          ${c.workspace_deps.length > 0
            ? `<ul class="dep-list">${c.workspace_deps.map(d => `
                <li>
                  <span class="dep-arrow">&rarr;</span>
                  <button class="dep-link" data-crate="${d}">${d}</button>
                </li>`).join('')}</ul>`
            : '<p style="color:var(--text2);font-size:0.85rem">No workspace dependencies</p>'}

          ${rdeps.length > 0 ? `
            <div class="panel-title" style="margin-top:1rem">Depended On By (${rdeps.length})</div>
            <ul class="dep-list">${rdeps.map(d => `
              <li>
                <span class="dep-arrow">&larr;</span>
                <button class="dep-link" data-crate="${d.name}">${d.name}</button>
              </li>`).join('')}</ul>` : ''}
        </div>

        <!-- Features -->
        ${featureEntries.length > 0 ? `
        <div class="detail-panel full-width">
          <div class="panel-title">Feature Flags</div>
          <ul class="type-list">${featureEntries.map(([k, v]) => {
            const vals = Array.isArray(v) && v.length > 0 ? ` = [${v.join(', ')}]` : '';
            return `<li><strong>${k}</strong>${vals}</li>`;
          }).join('')}</ul>
        </div>` : ''}

        <!-- Dependency mini-graph -->
        <div class="detail-panel full-width">
          <div class="panel-title">Dependency Context</div>
          <canvas id="detail-mini-graph" height="200" style="width:100%;border-radius:6px"></canvas>
        </div>
      </div>`;

    // Wire up events
    this.container.querySelector('#detail-back').addEventListener('click', () => this.app.back());

    this.container.querySelectorAll('.dep-link').forEach(link => {
      link.addEventListener('click', () => this.app.showCrate(link.dataset.crate));
    });

    // Draw mini dependency graph
    this._drawMiniGraph(c, rdeps);
  }

  _drawMiniGraph(center, rdeps) {
    const canvas = this.container.querySelector('#detail-mini-graph');
    if (!canvas) return;
    const ctx = canvas.getContext('2d');
    const dpr = window.devicePixelRatio || 1;
    const rect = canvas.getBoundingClientRect();
    const W = rect.width;
    const H = 200;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    canvas.style.height = `${H}px`;
    ctx.scale(dpr, dpr);

    const centerX = W / 2;
    const centerY = H / 2;

    // Position deps on left, rdeps on right
    const deps = center.workspace_deps.map(d => this.app.getCrate(d)).filter(Boolean);
    const nodes = [];

    // Center node
    const centerNode = { x: centerX, y: centerY, name: center.name, color: LAYER_COLORS[center.layer] || '#8b949e' };
    nodes.push(centerNode);

    // Dependencies (left side)
    const depSpacing = Math.min(35, (H - 40) / Math.max(deps.length, 1));
    const depStartY = centerY - ((deps.length - 1) * depSpacing) / 2;
    deps.forEach((d, i) => {
      nodes.push({
        x: centerX - 180,
        y: depStartY + i * depSpacing,
        name: d.name,
        color: LAYER_COLORS[d.layer] || '#8b949e',
        link: centerNode,
      });
    });

    // Reverse deps (right side)
    const rdepSpacing = Math.min(35, (H - 40) / Math.max(rdeps.length, 1));
    const rdepStartY = centerY - ((rdeps.length - 1) * rdepSpacing) / 2;
    rdeps.forEach((d, i) => {
      const n = {
        x: centerX + 180,
        y: rdepStartY + i * rdepSpacing,
        name: d.name,
        color: LAYER_COLORS[d.layer] || '#8b949e',
        rlink: centerNode,
      };
      nodes.push(n);
    });

    ctx.clearRect(0, 0, W, H);

    // Draw edges
    nodes.forEach(n => {
      if (n.link) {
        ctx.beginPath();
        ctx.moveTo(n.x, n.y);
        ctx.lineTo(n.link.x, n.link.y);
        ctx.strokeStyle = '#30363d';
        ctx.lineWidth = 1.5;
        ctx.stroke();
      }
      if (n.rlink) {
        ctx.beginPath();
        ctx.moveTo(n.x, n.y);
        ctx.lineTo(n.rlink.x, n.rlink.y);
        ctx.strokeStyle = '#30363d';
        ctx.lineWidth = 1.5;
        ctx.stroke();
      }
    });

    // Draw nodes
    nodes.forEach((n, i) => {
      const r = i === 0 ? 8 : 5;
      ctx.beginPath();
      ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
      ctx.fillStyle = n.color;
      ctx.fill();

      ctx.font = `${i === 0 ? '600 12' : '11'}px -apple-system, sans-serif`;
      ctx.fillStyle = '#e6edf3';
      ctx.textAlign = 'center';
      ctx.fillText(n.name, n.x, n.y - r - 4);
    });

    // Labels
    if (deps.length > 0) {
      ctx.font = '10px -apple-system, sans-serif';
      ctx.fillStyle = '#8b949e';
      ctx.textAlign = 'center';
      ctx.fillText('depends on', centerX - 90, 16);
    }
    if (rdeps.length > 0) {
      ctx.font = '10px -apple-system, sans-serif';
      ctx.fillStyle = '#8b949e';
      ctx.textAlign = 'center';
      ctx.fillText('used by', centerX + 90, 16);
    }

    // Click to navigate
    canvas.onclick = (e) => {
      const cr = canvas.getBoundingClientRect();
      const mx = e.clientX - cr.left;
      const my = e.clientY - cr.top;
      for (const n of nodes) {
        if (n.name !== center.name && Math.hypot(mx - n.x, my - n.y) < 15) {
          this.app.showCrate(n.name);
          return;
        }
      }
    };
    canvas.style.cursor = 'pointer';
  }
}
