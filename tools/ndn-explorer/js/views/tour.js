export class Tour {
  constructor(container, app) {
    this.container = container;
    this.app = app;
    this.current = 0;
    this.steps = [
      {
        title: 'Welcome to ndn-rs',
        body: `<strong>ndn-rs</strong> is a Named Data Networking forwarder stack written in Rust.
          NDN is a content-centric networking architecture: consumers express <strong>Interests</strong>
          by name, and the network routes them toward producers, returning <strong>Data</strong> along
          the reverse path with <strong>in-network caching</strong> at every hop.
          <br><br>This explorer helps you navigate the codebase — 31 crates across 10 architectural layers.`,
      },
      {
        title: 'Foundation: TLV and Packets',
        body: `Everything starts with <code>ndn-tlv</code> (wire encoding) and <code>ndn-packet</code>
          (Interest, Data, Name types). Both are <code>no_std</code> — they run on embedded sensors
          and full servers alike. Packets use <strong>lazy decoding</strong> via <code>OnceLock</code>:
          fields are only parsed when first accessed, so a cache hit can short-circuit before the nonce
          is ever decoded.`,
        action: { type: 'crate', target: 'ndn-packet' },
      },
      {
        title: 'The Face Abstraction',
        body: `<code>ndn-transport</code> defines the <code>Face</code> trait — async send/recv over
          any transport. UDP, TCP, raw Ethernet, serial, Bluetooth, WiFi broadcast, shared memory, and
          in-process channels all implement it. Each face runs its own Tokio task; one pipeline runner
          drains incoming packets.`,
        action: { type: 'crate', target: 'ndn-transport' },
      },
      {
        title: 'Forwarding Tables',
        body: `<code>ndn-store</code> provides the three core tables:
          <br>&bull; <strong>FIB</strong> — NameTrie with per-node RwLock for concurrent longest-prefix match
          <br>&bull; <strong>PIT</strong> — DashMap for sharded, lock-free Interest aggregation
          <br>&bull; <strong>Content Store</strong> — trait-based with LruCs, ShardedCs, and FjallCs (disk-backed)`,
        action: { type: 'crate', target: 'ndn-store' },
      },
      {
        title: 'The Pipeline',
        body: `Packets flow through <code>PipelineStage</code>s by value — ownership transfer makes
          short-circuits compiler-enforced. Each stage returns an <code>Action</code>: Continue, Send,
          Satisfy, Drop, or Nack.
          <br><br><strong>Interest:</strong> Decode → CS Lookup → PIT Check → Strategy → Dispatch
          <br><strong>Data:</strong> Decode → PIT Match → Validation → CS Insert → Dispatch`,
        action: { type: 'view', target: 'pipeline-trace' },
      },
      {
        title: 'Strategies',
        body: `<code>ndn-strategy</code> provides BestRoute, Multicast, ASF, and composed strategies.
          Strategies receive an immutable context and return forwarding decisions. They're hot-swappable
          at runtime and can even be loaded from <strong>WASM modules</strong> via <code>ndn-strategy-wasm</code>.
          A measurements table tracks EWMA RTT and satisfaction rates per face/prefix.`,
        action: { type: 'crate', target: 'ndn-strategy' },
      },
      {
        title: 'The Engine',
        body: `<code>ndn-engine</code> wires everything together. <code>EngineBuilder</code> configures
          the ForwarderEngine with face tasks, pipeline stages, strategy table, and expiry timers. The
          engine is a <strong>library</strong>, not a daemon — embed it in any Rust application, or run
          it standalone via <code>ndn-router</code>.`,
        action: { type: 'crate', target: 'ndn-engine' },
      },
      {
        title: 'Simulation',
        body: `<code>ndn-sim</code> lets you build multi-node topologies entirely in-process.
          <code>SimFace</code> implements the Face trait via Tokio channels with configurable delay,
          loss, bandwidth, and jitter. The <code>Simulation</code> topology builder creates nodes,
          links them, installs FIB routes, and starts all engines. No Mininet needed.`,
        action: { type: 'crate', target: 'ndn-sim' },
      },
      {
        title: 'Explore!',
        body: `You've seen the highlights. Now explore on your own:
          <br>&bull; <strong>Layers</strong> — browse all 31 crates by architectural layer
          <br>&bull; <strong>Graph</strong> — interactive dependency visualization with hover highlighting
          <br>&bull; <strong>Pipeline</strong> — step through Interest/Data processing stage by stage
          <br>&bull; <strong>Search</strong> — find any crate, type, or feature (or press <code>/</code>)`,
        action: { type: 'view', target: 'layer-map' },
      },
    ];
  }

  render() {
    this._renderStep();
  }

  onShow() {
    // No-op, already rendered
  }

  _renderStep() {
    const s = this.steps[this.current];
    const total = this.steps.length;
    const pct = ((this.current + 1) / total * 100).toFixed(0);

    this.container.innerHTML = `
      <h1 style="margin-bottom:0.75rem">Guided Tour</h1>
      <div class="tour-progress">
        Step ${this.current + 1} of ${total}
        <div class="tour-progress-bar">
          <div class="tour-progress-fill" style="width:${pct}%"></div>
        </div>
      </div>
      <div class="tour-card">
        <h2>${s.title}</h2>
        <p>${s.body}</p>
        ${s.action ? `<button class="tour-action" id="tour-action-btn">
          ${s.action.type === 'crate' ? `View ${s.action.target}` : `Open ${s.action.target}`} &rarr;
        </button>` : ''}
      </div>
      <div class="tour-nav">
        ${this.current > 0 ? '<button class="tour-btn tour-btn-secondary" data-dir="prev">&larr; Previous</button>' : ''}
        ${this.current < total - 1
          ? '<button class="tour-btn tour-btn-primary" data-dir="next">Next &rarr;</button>'
          : '<button class="tour-btn tour-btn-primary" data-dir="finish">Start Exploring</button>'}
      </div>`;

    // Wire action button
    const actionBtn = this.container.querySelector('#tour-action-btn');
    if (actionBtn && s.action) {
      actionBtn.addEventListener('click', () => {
        if (s.action.type === 'crate') this.app.showCrate(s.action.target);
        else this.app.navigate(s.action.target);
      });
    }

    // Wire nav buttons
    this.container.querySelectorAll('.tour-nav button').forEach(btn => {
      btn.addEventListener('click', () => {
        if (btn.dataset.dir === 'next') {
          this.current++;
          this._renderStep();
        } else if (btn.dataset.dir === 'prev') {
          this.current--;
          this._renderStep();
        } else {
          this.app.navigate('layer-map');
        }
      });
    });
  }
}
