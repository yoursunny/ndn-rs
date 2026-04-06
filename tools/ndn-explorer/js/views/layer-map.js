import { LAYER_COLORS } from '../app.js';

export class LayerMap {
  constructor(container, app) {
    this.container = container;
    this.app = app;
  }

  render() {
    const { data } = this.app;
    const layers = [...data.layers].sort((a, b) => b.num - a.num);

    this.container.innerHTML = layers.map(layer => {
      const crates = data.crates.filter(c => c.layer === layer.id);
      if (crates.length === 0) return '';
      const color = LAYER_COLORS[layer.id] || '#8b949e';

      return `
        <div class="layer-section">
          <div class="layer-header" style="background:${color}12">
            <span class="layer-dot" style="background:${color}"></span>
            Layer ${layer.num} &mdash; ${layer.label}
            <span style="margin-left:auto;font-size:0.72rem;color:var(--text2);font-weight:400">${crates.length} crate${crates.length > 1 ? 's' : ''}</span>
          </div>
          <div class="layer-body">
            ${crates.map(c => this._card(c, color)).join('')}
          </div>
        </div>`;
    }).join('');

    this.container.querySelectorAll('.crate-card').forEach(card => {
      card.addEventListener('click', () => this.app.showCrate(card.dataset.crate));
    });
  }

  _card(c, layerColor) {
    const depCount = c.workspace_deps.length;
    const typeCount = c.key_types.length;
    return `
      <div class="crate-card" data-crate="${c.name}" style="border-left:3px solid ${layerColor}">
        <h3>${c.name}</h3>
        <div class="desc">${esc(c.description)}</div>
        <div class="badges">
          ${c.no_std ? '<span class="badge badge-green">no_std</span>' : ''}
          ${typeCount > 0 ? `<span class="badge">${typeCount} types</span>` : ''}
          ${depCount > 0 ? `<span class="badge">${depCount} deps</span>` : ''}
          ${Object.keys(c.features).length > 0 ? `<span class="badge">features</span>` : ''}
        </div>
      </div>`;
  }
}

function esc(s) {
  return s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
}
