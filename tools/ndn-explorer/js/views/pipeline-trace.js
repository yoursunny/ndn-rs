const PIPELINES = {
  interest: {
    label: 'Interest',
    cssClass: 'pkt-interest',
    stages: [
      {
        name: 'TlvDecode',
        crate: 'ndn-engine',
        desc: 'Decode raw bytes into an Interest packet. Unwrap NDNLPv2 link-protocol headers (fragmentation reassembly, Nack detection). Enforce /localhost scope — drop if the Interest arrived on a non-local face but targets a local prefix.',
        actions: ['Continue', 'Drop'],
      },
      {
        name: 'CsLookup',
        crate: 'ndn-engine',
        desc: 'Search the Content Store for cached Data matching this Interest (name + CanBePrefix + MustBeFresh). On a cache hit, short-circuit: return the cached Data immediately to the downstream face without touching PIT or FIB. On miss, continue to PIT.',
        actions: ['Satisfy', 'Continue'],
      },
      {
        name: 'PitCheck',
        crate: 'ndn-engine',
        desc: 'Check the Pending Interest Table. If an existing PIT entry matches (same name + selectors), aggregate this Interest by adding the incoming face to the in-record list — the Interest is not forwarded again. If no match, create a new PIT entry and continue to the strategy.',
        actions: ['Continue', 'Drop'],
      },
      {
        name: 'Strategy',
        crate: 'ndn-engine',
        desc: 'Consult the forwarding strategy assigned to this name prefix. The strategy performs FIB longest-prefix-match, selects outgoing face(s) based on measurements (RTT, satisfaction rate), and returns a forwarding decision. May also schedule a ForwardAfter probe to a secondary face.',
        actions: ['Send', 'Nack'],
      },
    ],
  },
  data: {
    label: 'Data',
    cssClass: 'pkt-data',
    stages: [
      {
        name: 'TlvDecode',
        crate: 'ndn-engine',
        desc: 'Decode raw bytes into a Data packet. Unwrap NDNLPv2 link-protocol headers. Enforce /localhost scope — drop if the Data targets a local prefix but arrived from a non-local face.',
        actions: ['Continue', 'Drop'],
      },
      {
        name: 'PitMatch',
        crate: 'ndn-engine',
        desc: 'Match this Data against PIT entries by name (and selectors). Collect all downstream faces from matching in-records — these are the consumers waiting for this Data. If no PIT entry matches, this is unsolicited Data and gets dropped.',
        actions: ['Continue', 'Drop'],
      },
      {
        name: 'Validation',
        crate: 'ndn-engine',
        desc: 'Verify the Data packet\'s cryptographic signature. Walk the certificate chain if needed, fetching missing certificates via the CertFetcher. Packets awaiting certificates are queued. If validation fails (bad signature, untrusted key, expired cert), the Data is dropped.',
        actions: ['Continue', 'Drop'],
      },
      {
        name: 'CsInsert',
        crate: 'ndn-engine',
        desc: 'Insert the validated Data into the Content Store. The admission policy decides whether to cache (e.g., skip if CachePolicyType::NoCache is set in LP headers). Then dispatch: send the Data to all downstream faces collected from PIT in-records.',
        actions: ['Send'],
      },
    ],
  },
};

const ACTION_CLASSES = {
  Continue: 'action-continue',
  Send: 'action-send',
  Satisfy: 'action-satisfy',
  Drop: 'action-drop',
  Nack: 'action-nack',
};

export class PipelineTrace {
  constructor(container, app) {
    this.container = container;
    this.app = app;
    this.activeStage = { interest: -1, data: -1 };
  }

  render() {
    this.container.innerHTML = `
      <h1 style="margin-bottom:0.5rem">Pipeline Trace</h1>
      <p style="color:var(--text2);font-size:0.85rem;margin-bottom:1.5rem">
        Step through the Interest and Data processing pipelines. Each stage receives a <code>PacketContext</code> by value and returns an <code>Action</code>.
      </p>
      ${this._renderPipeline('interest')}
      ${this._renderPipeline('data')}
    `;
    this._wireEvents();
  }

  _renderPipeline(type) {
    const p = PIPELINES[type];
    return `
      <div class="pipeline-section" id="pipeline-${type}">
        <div class="pipeline-heading">
          <span class="pkt-badge ${p.cssClass}">${p.label}</span>
          Pipeline
        </div>
        <div class="pipeline-controls">
          <button class="ctrl-btn" data-action="prev" data-type="${type}">&larr; Prev</button>
          <button class="ctrl-btn" data-action="next" data-type="${type}">Next &rarr;</button>
          <button class="ctrl-btn" data-action="reset" data-type="${type}">Reset</button>
          <button class="ctrl-btn" data-action="play" data-type="${type}">&#9654; Auto</button>
        </div>
        <div class="stage-flow">
          ${p.stages.map((s, i) => `
            <div class="stage-box" data-type="${type}" data-idx="${i}">
              <div class="stage-num">Stage ${i + 1}</div>
              <h4>${s.name}</h4>
              <div class="stage-crate">${s.crate}</div>
            </div>
            ${i < p.stages.length - 1 ? '<div class="stage-arrow">&rarr;</div>' : ''}
          `).join('')}
        </div>
        <div class="stage-detail" id="detail-${type}">
          <h3>Select a stage</h3>
          <p>Click a stage above or use the controls to step through the ${p.label.toLowerCase()} processing pipeline.</p>
        </div>
      </div>`;
  }

  _wireEvents() {
    this.container.querySelectorAll('.stage-box').forEach(box => {
      box.addEventListener('click', () => {
        this._setActive(box.dataset.type, parseInt(box.dataset.idx));
      });
    });

    this.container.querySelectorAll('.ctrl-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        const type = btn.dataset.type;
        const action = btn.dataset.action;
        const p = PIPELINES[type];
        if (action === 'next') {
          this._setActive(type, Math.min(this.activeStage[type] + 1, p.stages.length - 1));
        } else if (action === 'prev') {
          this._setActive(type, Math.max(this.activeStage[type] - 1, -1));
        } else if (action === 'reset') {
          this._setActive(type, -1);
        } else if (action === 'play') {
          this._autoPlay(type);
        }
      });
    });
  }

  _setActive(type, idx) {
    this.activeStage[type] = idx;
    const p = PIPELINES[type];

    // Update stage boxes
    this.container.querySelectorAll(`.stage-box[data-type="${type}"]`).forEach((box, i) => {
      box.classList.remove('active', 'visited');
      if (i === idx) box.classList.add('active');
      else if (i < idx) box.classList.add('visited');
    });

    // Update detail panel
    const detail = this.container.querySelector(`#detail-${type}`);
    if (idx >= 0 && idx < p.stages.length) {
      const s = p.stages[idx];
      detail.innerHTML = `
        <h3>Stage ${idx + 1}: ${s.name}</h3>
        <p>${s.desc}</p>
        <div class="action-list">
          <span style="font-size:0.72rem;color:var(--text2);margin-right:0.2rem">Possible outcomes:</span>
          ${s.actions.map(a => `<span class="action-tag ${ACTION_CLASSES[a] || ''}">${a}</span>`).join('')}
        </div>`;
    } else {
      detail.innerHTML = `
        <h3>Select a stage</h3>
        <p>Click a stage above or use the controls to step through the ${p.label.toLowerCase()} processing pipeline.</p>`;
    }
  }

  _autoPlay(type) {
    const p = PIPELINES[type];
    this._setActive(type, -1);
    let i = 0;
    const timer = setInterval(() => {
      this._setActive(type, i);
      i++;
      if (i >= p.stages.length) clearInterval(timer);
    }, 800);
  }
}
