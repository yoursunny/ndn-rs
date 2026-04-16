import { LAYER_COLORS } from '../app.js';

/**
 * Zone-based crate map.  Crates are grouped by zone (Core, Applications,
 * Extensions, Targets, Examples) instead of the old negative-number layer
 * scheme.  Within each zone, layers are listed top-to-bottom by depth.
 */
export class LayerMap {
  constructor(container, app) {
    this.container = container;
    this.app = app;
  }

  render() {
    const { data } = this.app;
    const zones = data.zones
      ? [...data.zones].sort((a, b) => a.order - b.order)
      : this._inferZones(data.layers);

    this.container.innerHTML = zones.map(zone => {
      // Collect all layers belonging to this zone, sorted by zone_depth
      const zoneLayers = data.layers
        .filter(l => l.zone === zone.id)
        .sort((a, b) => (a.zone_depth ?? a.num) - (b.zone_depth ?? b.num));

      const zoneCrates = data.crates.filter(c =>
        zoneLayers.some(l => l.id === c.layer)
      );
      if (zoneCrates.length === 0) return '';

      const layerSections = zoneLayers.map(layer => {
        const crates = data.crates.filter(c => c.layer === layer.id);
        if (crates.length === 0) return '';
        const color = LAYER_COLORS[layer.id] || '#8b949e';

        return `
          <div class="layer-group">
            <div class="layer-header" style="background:${color}12">
              <span class="layer-dot" style="background:${color}"></span>
              ${layer.label}
              <span style="margin-left:auto;font-size:0.72rem;color:var(--text2);font-weight:400">${crates.length} crate${crates.length > 1 ? 's' : ''}</span>
            </div>
            <div class="layer-body">
              ${crates.map(c => this._card(c, color)).join('')}
            </div>
          </div>`;
      }).join('');

      return `
        <div class="zone-section">
          <div class="zone-header">
            <h2 class="zone-title">${zone.label}</h2>
            ${zone.description ? `<span class="zone-desc">${esc(zone.description)}</span>` : ''}
            <span class="zone-count">${zoneCrates.length} crate${zoneCrates.length > 1 ? 's' : ''}</span>
          </div>
          ${layerSections}
        </div>`;
    }).join('');

    this.container.querySelectorAll('.crate-card').forEach(card => {
      card.addEventListener('click', () => this.app.showCrate(card.dataset.crate));
    });
  }

  /** Backwards-compat: infer zones from layer nums if zones array is absent. */
  _inferZones(layers) {
    const zoneMap = new Map();
    for (const l of layers) {
      const zid = l.zone || (l.num > 0 ? 'core' : l.num === 0 ? 'apps' : 'extensions');
      if (!zoneMap.has(zid)) {
        zoneMap.set(zid, { id: zid, label: zid.charAt(0).toUpperCase() + zid.slice(1), order: zoneMap.size });
      }
    }
    return [...zoneMap.values()];
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
