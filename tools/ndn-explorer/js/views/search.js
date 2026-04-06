import { LAYER_COLORS } from '../app.js';

export class Search {
  constructor(container, app) {
    this.container = container;
    this.app = app;
    this.index = null;
  }

  _buildIndex() {
    if (this.index) return;
    this.index = [];
    this.app.data.crates.forEach(c => {
      this.index.push({
        kind: 'crate',
        name: c.name,
        desc: c.description,
        crate: c.name,
        layer: c.layer,
      });
      c.key_types.forEach(t => {
        this.index.push({
          kind: 'type',
          name: t,
          desc: `Exported by ${c.name}`,
          crate: c.name,
          layer: c.layer,
        });
      });
      Object.keys(c.features).forEach(f => {
        this.index.push({
          kind: 'feature',
          name: f,
          desc: `Feature flag in ${c.name}`,
          crate: c.name,
          layer: c.layer,
        });
      });
    });
  }

  search(query) {
    this._buildIndex();
    const q = query.toLowerCase();
    const terms = q.split(/\s+/).filter(Boolean);

    // Score each index entry
    const scored = this.index.map(item => {
      let score = 0;
      const nameLower = item.name.toLowerCase();
      const descLower = item.desc.toLowerCase();

      for (const term of terms) {
        if (nameLower === term) score += 100;
        else if (nameLower.startsWith(term)) score += 60;
        else if (nameLower.includes(term)) score += 30;
        if (descLower.includes(term)) score += 10;
      }

      // Boost crates over types
      if (item.kind === 'crate') score += 5;

      return { item, score };
    }).filter(s => s.score > 0)
      .sort((a, b) => b.score - a.score)
      .slice(0, 30);

    this._renderResults(query, scored.map(s => s.item));
  }

  _renderResults(query, results) {
    const esc = s => s.replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');

    this.container.innerHTML = `
      <h1 style="margin-bottom:0.25rem">Search</h1>
      <div class="search-count">${results.length} result${results.length !== 1 ? 's' : ''} for "${esc(query)}"</div>
      ${results.length === 0 ? '<p style="color:var(--text2);margin-top:1rem">No matches found. Try a crate name, type, or keyword.</p>' : ''}
      ${results.map(r => {
        const color = LAYER_COLORS[r.layer] || '#8b949e';
        return `
          <div class="search-result" data-crate="${r.crate}">
            <span class="match-type" style="color:${color}">${r.kind}</span>
            <span class="match-name">${this._highlight(r.name, query)}</span>
            <span class="match-in">${r.crate}</span>
          </div>`;
      }).join('')}`;

    this.container.querySelectorAll('.search-result').forEach(el => {
      el.addEventListener('click', () => this.app.showCrate(el.dataset.crate));
    });
  }

  _highlight(text, query) {
    const terms = query.toLowerCase().split(/\s+/).filter(Boolean);
    let result = text;
    for (const term of terms) {
      const idx = result.toLowerCase().indexOf(term);
      if (idx >= 0) {
        const before = result.slice(0, idx);
        const match = result.slice(idx, idx + term.length);
        const after = result.slice(idx + term.length);
        result = `${before}<mark style="background:rgba(88,166,255,0.2);color:var(--accent);border-radius:2px;padding:0 1px">${match}</mark>${after}`;
        break; // Only highlight first match to avoid nested marks
      }
    }
    return result;
  }
}
