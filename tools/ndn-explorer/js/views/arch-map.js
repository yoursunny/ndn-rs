import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { CSS2DRenderer, CSS2DObject } from 'three/addons/renderers/CSS2DRenderer.js';
import { LAYER_COLORS } from '../app.js';

// ── Constants ────────────────────────────────────────────────────────────────

const ZONE_RADII = { core: 8, apps: 12, extensions: 16, targets: 20, examples: 24 };
const ZONE_COLORS = {
  core:       0x58a6ff,
  apps:       0xf0883e,
  extensions: 0xff7b72,
  targets:    0x7ee787,
  examples:   0x8b949e,
};

const CIRCUIT_COLORS = {
  trace:       0xc87533,   // copper
  trace_glow:  0xffaa55,   // lit copper
  ic_chip:     0x2d333b,
  memory_chip: 0x1a2332,
  logic_gate:  0x332244,
  filter:      0x2d3322,
  capacitor:   0x223344,
  bus:         0x443322,
  board:       0x0a1628,   // dark PCB green-blue
  interest:    0x58a6ff,   // blue
  data:        0x3fb950,   // green
  nack:        0xff7b72,   // red
};

const STAGE_POSITIONS = {
  decode:       { x: -10, y: 0 },
  cs_lookup:    { x: -5,  y: 3 },
  pit_check:    { x: 0,   y: 0 },
  strategy:     { x: 5,   y: 0 },
  validation:   { x: -5,  y: -3 },
  pit_match:    { x: 0,   y: -3 },
  cs_insert:    { x: 5,   y: -3 },
  nack_strategy:{ x: 5,   y: 3 },
};

// ── Architecture Map View ────────────────────────────────────────────────────

export class ArchMap {
  constructor(container, app) {
    this.container = container;
    this.app = app;
    this.scene = null;
    this.camera = null;
    this.renderer = null;
    this.labelRenderer = null;
    this.controls = null;
    this.raycaster = new THREE.Raycaster();
    this.mouse = new THREE.Vector2();
    this.level = 1;          // 1=galaxy, 2=circuit, 3=deep dive
    this.hoveredObject = null;
    this.crateNodes = [];
    this.chipMeshes = [];
    this.traceLines = [];
    this.particles = [];
    this.animating = false;
    this.animationId = null;
    this.tooltipEl = null;
    this.popupEl = null;
    this.engineData = null;
    this._built = false;
  }

  onShow() {
    if (!this._built) {
      this._build();
      this._built = true;
    }
    this._resize();
    if (!this.animating) this._startAnimation();
  }

  _build() {
    this.engineData = this.app.engineData;

    // Scene
    this.scene = new THREE.Scene();
    this.scene.background = new THREE.Color(0x0d1117);
    this.scene.fog = new THREE.FogExp2(0x0d1117, 0.008);

    // Camera
    this.camera = new THREE.PerspectiveCamera(60, 1, 0.1, 200);
    this.camera.position.set(0, 0, 40);

    // Renderer
    this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false });
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    this.container.appendChild(this.renderer.domElement);

    // CSS2D label renderer
    this.labelRenderer = new CSS2DRenderer();
    this.labelRenderer.domElement.style.position = 'absolute';
    this.labelRenderer.domElement.style.top = '0';
    this.labelRenderer.domElement.style.left = '0';
    this.labelRenderer.domElement.style.pointerEvents = 'none';
    this.container.style.position = 'relative';
    this.container.appendChild(this.labelRenderer.domElement);

    // Controls
    this.controls = new OrbitControls(this.camera, this.renderer.domElement);
    this.controls.enableDamping = true;
    this.controls.dampingFactor = 0.08;
    this.controls.minDistance = 5;
    this.controls.maxDistance = 80;

    // Lights
    this.scene.add(new THREE.AmbientLight(0x404060, 0.6));
    const dirLight = new THREE.DirectionalLight(0xffffff, 0.8);
    dirLight.position.set(10, 20, 15);
    this.scene.add(dirLight);
    const pointLight = new THREE.PointLight(0x58a6ff, 0.4, 60);
    pointLight.position.set(0, 0, 10);
    this.scene.add(pointLight);

    // Tooltip element
    this.tooltipEl = document.createElement('div');
    this.tooltipEl.className = 'arch-tooltip';
    this.tooltipEl.style.display = 'none';
    this.container.appendChild(this.tooltipEl);

    // Popup element (for click details)
    this.popupEl = document.createElement('div');
    this.popupEl.className = 'arch-popup';
    this.popupEl.style.display = 'none';
    this.container.appendChild(this.popupEl);

    // Build Level 1 (Galaxy)
    this._buildGalaxyView();

    // Build Level 2 (Circuit) — hidden initially
    this._buildCircuitBoard();

    // Events
    this.renderer.domElement.addEventListener('mousemove', e => this._onMouseMove(e));
    this.renderer.domElement.addEventListener('click', e => this._onClick(e));
    window.addEventListener('resize', () => this._resize());

    // Toolbar
    this._buildToolbar();

    this._resize();
  }

  // ── Level 1: Galaxy View ─────────────────────────────────────────────────

  _buildGalaxyView() {
    this.galaxyGroup = new THREE.Group();
    this.galaxyGroup.name = 'galaxy';
    this.scene.add(this.galaxyGroup);

    const zones = this.app.data.zones || [];
    const crates = this.app.data.crates || [];
    const layers = this.app.data.layers || [];

    // Zone shells (concentric translucent spheres)
    for (const zone of zones) {
      const radius = ZONE_RADII[zone.id] || 15;
      const color = ZONE_COLORS[zone.id] || 0x8b949e;
      const shellGeo = new THREE.SphereGeometry(radius, 32, 24);
      const shellMat = new THREE.MeshPhongMaterial({
        color, transparent: true, opacity: 0.04,
        side: THREE.DoubleSide, depthWrite: false,
      });
      const shell = new THREE.Mesh(shellGeo, shellMat);
      shell.userData = { type: 'zone', zone };
      this.galaxyGroup.add(shell);

      // Zone label
      const label = this._makeLabel(zone.label, 0.7, '#8b949e');
      label.position.set(0, radius + 0.5, 0);
      this.galaxyGroup.add(label);
    }

    // Crate nodes — positioned within their zone shell
    const zoneCrateCounts = {};
    for (const c of crates) {
      const layer = layers.find(l => l.id === c.layer);
      const zoneId = layer?.zone || 'core';
      if (!zoneCrateCounts[zoneId]) zoneCrateCounts[zoneId] = 0;
      zoneCrateCounts[zoneId]++;
    }

    const zoneCounters = {};
    for (const c of crates) {
      const layer = layers.find(l => l.id === c.layer);
      const zoneId = layer?.zone || 'core';
      const radius = ZONE_RADII[zoneId] || 15;
      const color = LAYER_COLORS[c.layer] || '#8b949e';

      if (!zoneCounters[zoneId]) zoneCounters[zoneId] = 0;
      const index = zoneCounters[zoneId]++;
      const total = zoneCrateCounts[zoneId];

      // Fibonacci sphere distribution within zone shell
      const phi = Math.acos(1 - 2 * (index + 0.5) / total);
      const theta = Math.PI * (1 + Math.sqrt(5)) * index;
      const r = radius * 0.7 + (layer?.zone_depth || 0) * 1.5;
      const x = r * Math.sin(phi) * Math.cos(theta);
      const y = r * Math.sin(phi) * Math.sin(theta);
      const z = r * Math.cos(phi);

      // Node sphere
      const nodeGeo = new THREE.SphereGeometry(0.4, 16, 12);
      const nodeMat = new THREE.MeshPhongMaterial({
        color: new THREE.Color(color),
        emissive: new THREE.Color(color),
        emissiveIntensity: 0.3,
      });
      const node = new THREE.Mesh(nodeGeo, nodeMat);
      node.position.set(x, y, z);
      node.userData = { type: 'crate', crate: c, color };
      this.galaxyGroup.add(node);
      this.crateNodes.push(node);

      // Crate name label
      const nameLabel = this._makeLabel(c.name, 0.45, color);
      nameLabel.position.set(x, y + 0.7, z);
      this.galaxyGroup.add(nameLabel);
    }

    // Dependency edges
    for (const c of crates) {
      const srcNode = this.crateNodes.find(n => n.userData.crate?.name === c.name);
      if (!srcNode) continue;
      for (const dep of c.workspace_deps) {
        const tgtNode = this.crateNodes.find(n => n.userData.crate?.name === dep);
        if (!tgtNode) continue;
        const points = [srcNode.position.clone(), tgtNode.position.clone()];
        const lineGeo = new THREE.BufferGeometry().setFromPoints(points);
        const lineMat = new THREE.LineBasicMaterial({
          color: 0x30363d, transparent: true, opacity: 0.25,
        });
        const line = new THREE.Line(lineGeo, lineMat);
        line.userData = { type: 'dep-edge', from: c.name, to: dep };
        this.galaxyGroup.add(line);
      }
    }
  }

  // ── Level 2: Engine Circuit Board ────────────────────────────────────────

  _buildCircuitBoard() {
    this.circuitGroup = new THREE.Group();
    this.circuitGroup.name = 'circuit';
    this.circuitGroup.visible = false;
    this.scene.add(this.circuitGroup);

    if (!this.engineData) return;

    // PCB board background
    const boardGeo = new THREE.PlaneGeometry(30, 20);
    const boardMat = new THREE.MeshPhongMaterial({
      color: CIRCUIT_COLORS.board, side: THREE.DoubleSide,
    });
    const board = new THREE.Mesh(boardGeo, boardMat);
    board.position.z = -0.5;
    this.circuitGroup.add(board);

    // Grid lines on board (PCB trace pattern)
    this._drawPCBGrid();

    // Pipeline stage chips
    const stages = this.engineData.pipeline?.stages || {};
    for (const [id, stage] of Object.entries(stages)) {
      const pos = STAGE_POSITIONS[id];
      if (!pos) continue;
      this._buildChip(id, stage, pos.x, pos.y);
    }

    // Trace connections (copper traces between chips)
    this._buildTraces();

    // Face connectors around the perimeter
    this._buildFaceConnectors();

    // Table visualizations
    this._buildTableChips();

    // Security subsystem
    this._buildSecurityComponents();

    // Performance annotations
    this._buildPerformanceCallouts();

    // Animated packet control buttons
    this._buildPacketControls();
  }

  _drawPCBGrid() {
    const gridMat = new THREE.LineBasicMaterial({ color: 0x152238, transparent: true, opacity: 0.5 });
    for (let x = -14; x <= 14; x += 2) {
      const points = [new THREE.Vector3(x, -9, -0.4), new THREE.Vector3(x, 9, -0.4)];
      this.circuitGroup.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), gridMat));
    }
    for (let y = -9; y <= 9; y += 2) {
      const points = [new THREE.Vector3(-14, y, -0.4), new THREE.Vector3(14, y, -0.4)];
      this.circuitGroup.add(new THREE.Line(new THREE.BufferGeometry().setFromPoints(points), gridMat));
    }
  }

  _buildChip(id, stage, x, y) {
    // Chip body (rounded rectangle approximation)
    const chipW = 3.5, chipH = 2;
    const chipGeo = new THREE.BoxGeometry(chipW, chipH, 0.2);
    const chipColor = CIRCUIT_COLORS[stage.circuit_type] || CIRCUIT_COLORS.ic_chip;
    const chipMat = new THREE.MeshPhongMaterial({
      color: chipColor,
      emissive: chipColor,
      emissiveIntensity: 0.1,
    });
    const chip = new THREE.Mesh(chipGeo, chipMat);
    chip.position.set(x, y, 0);
    chip.userData = {
      type: 'chip',
      stageId: id,
      stage,
      interactive: true,
    };
    this.circuitGroup.add(chip);
    this.chipMeshes.push(chip);

    // Chip label
    const chipLabel = this._makeLabel(stage.chip_label, 0.5, '#e6edf3');
    chipLabel.position.set(x, y + 0.3, 0.2);
    this.circuitGroup.add(chipLabel);

    // Trait label (smaller, below main label)
    if (stage.trait) {
      const traitLabel = this._makeLabel(stage.trait, 0.32, '#8b949e');
      traitLabel.position.set(x, y - 0.3, 0.2);
      this.circuitGroup.add(traitLabel);
    }

    // Pin markers (input/output edges)
    const pinGeo = new THREE.CircleGeometry(0.1, 8);
    const pinMatIn = new THREE.MeshBasicMaterial({ color: 0x58a6ff });
    const pinMatOut = new THREE.MeshBasicMaterial({ color: 0x3fb950 });

    // Input pins (left side)
    const reads = stage.reads || [];
    reads.forEach((field, i) => {
      const pin = new THREE.Mesh(pinGeo, pinMatIn);
      pin.position.set(x - chipW / 2 - 0.15, y + 0.3 - i * 0.35, 0.15);
      pin.userData = { type: 'pin', direction: 'in', field, stageId: id };
      this.circuitGroup.add(pin);
    });

    // Output pins (right side)
    const writes = stage.writes || [];
    writes.forEach((field, i) => {
      const pin = new THREE.Mesh(pinGeo, pinMatOut);
      pin.position.set(x + chipW / 2 + 0.15, y + 0.3 - i * 0.35, 0.15);
      pin.userData = { type: 'pin', direction: 'out', field, stageId: id };
      this.circuitGroup.add(pin);
    });

    // Short-circuit LED indicators
    const shorts = stage.short_circuits || [];
    if (shorts.length > 0) {
      const ledGeo = new THREE.CircleGeometry(0.08, 8);
      shorts.forEach((sc, i) => {
        const ledColor = sc.action === 'Drop' ? 0xff4444 : sc.action === 'Satisfy' ? 0x44ff44 : 0xffaa44;
        const ledMat = new THREE.MeshBasicMaterial({ color: ledColor });
        const led = new THREE.Mesh(ledGeo, ledMat);
        led.position.set(x + chipW / 2 - 0.3 - i * 0.25, y - chipH / 2 + 0.15, 0.15);
        led.userData = { type: 'led', shortCircuit: sc, stageId: id };
        this.circuitGroup.add(led);
      });
    }

    // Timescale bar — proportional width below chip showing latency from CI benchmarks
    if (stage.timescale) {
      const ts = stage.timescale;
      // Pick the most representative number for the bar
      const primaryNs = parseTimescale(
        ts.interest_4comp || ts.hit || ts.new_entry || ts.fib_lpm_100 ||
        ts.disabled_passthrough || ts.cold_insert || Object.values(ts)[0]
      );
      // Normalize: 2000ns (2µs) = full chip width. Validation can exceed this.
      const maxNs = 2000;
      const barW = Math.min(chipW, (primaryNs / maxNs) * chipW);
      const barGeo = new THREE.PlaneGeometry(barW, 0.12);
      const barColor = primaryNs < 800 ? 0x3fb950 : primaryNs < 1500 ? 0xd29922 : 0xff7b72;
      const barMat = new THREE.MeshBasicMaterial({ color: barColor, transparent: true, opacity: 0.8 });
      const bar = new THREE.Mesh(barGeo, barMat);
      bar.position.set(x - chipW / 2 + barW / 2, y - chipH / 2 - 0.15, 0.15);
      this.circuitGroup.add(bar);

      // Time label
      const timeStr = ts.interest_4comp || ts.hit || ts.new_entry || ts.fib_lpm_100 ||
        ts.disabled_passthrough || ts.cold_insert || Object.values(ts)[0];
      const timeLbl = this._makeLabel(timeStr, 0.22, barColor === 0x3fb950 ? '#3fb950' : barColor === 0xd29922 ? '#d29922' : '#ff7b72');
      timeLbl.position.set(x, y - chipH / 2 - 0.35, 0.15);
      this.circuitGroup.add(timeLbl);
    }
  }

  _buildTraces() {
    const traceMat = new THREE.LineBasicMaterial({
      color: CIRCUIT_COLORS.trace, linewidth: 2,
    });

    // Interest path traces
    const interestPath = this.engineData.pipeline?.interest_path || [];
    for (let i = 0; i < interestPath.length - 1; i++) {
      const from = STAGE_POSITIONS[interestPath[i]];
      const to = STAGE_POSITIONS[interestPath[i + 1]];
      if (!from || !to) continue;

      const mid = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
      const curve = new THREE.QuadraticBezierCurve3(
        new THREE.Vector3(from.x + 1.75, from.y, 0.05),
        new THREE.Vector3(mid.x, mid.y + 0.5, 0.05),
        new THREE.Vector3(to.x - 1.75, to.y, 0.05),
      );
      const points = curve.getPoints(20);
      const geo = new THREE.BufferGeometry().setFromPoints(points);
      const line = new THREE.Line(geo, traceMat.clone());
      line.userData = { type: 'trace', path: 'interest', from: interestPath[i], to: interestPath[i + 1] };
      this.circuitGroup.add(line);
      this.traceLines.push(line);
    }

    // Data path traces
    const dataPath = this.engineData.pipeline?.data_path || [];
    const dataTraceMat = new THREE.LineBasicMaterial({
      color: CIRCUIT_COLORS.trace, linewidth: 2,
    });
    for (let i = 0; i < dataPath.length - 1; i++) {
      const from = STAGE_POSITIONS[dataPath[i]];
      const to = STAGE_POSITIONS[dataPath[i + 1]];
      if (!from || !to) continue;

      const mid = { x: (from.x + to.x) / 2, y: (from.y + to.y) / 2 };
      const curve = new THREE.QuadraticBezierCurve3(
        new THREE.Vector3(from.x + 1.75, from.y, 0.05),
        new THREE.Vector3(mid.x, mid.y - 0.5, 0.05),
        new THREE.Vector3(to.x - 1.75, to.y, 0.05),
      );
      const points = curve.getPoints(20);
      const geo = new THREE.BufferGeometry().setFromPoints(points);
      const line = new THREE.Line(geo, dataTraceMat.clone());
      line.userData = { type: 'trace', path: 'data', from: dataPath[i], to: dataPath[i + 1] };
      this.circuitGroup.add(line);
      this.traceLines.push(line);
    }

    // Short-circuit trace: CS hit -> Satisfy (bypass PIT/FIB/strategy)
    const csPos = STAGE_POSITIONS.cs_lookup;
    if (csPos) {
      const bypassCurve = new THREE.QuadraticBezierCurve3(
        new THREE.Vector3(csPos.x, csPos.y - 1, 0.05),
        new THREE.Vector3(csPos.x + 6, csPos.y - 5, 0.05),
        new THREE.Vector3(-12, csPos.y - 2, 0.05),   // exits toward face connectors
      );
      const points = bypassCurve.getPoints(20);
      const geo = new THREE.BufferGeometry().setFromPoints(points);
      const bypassMat = new THREE.LineDashedMaterial({
        color: 0x3fb950, dashSize: 0.3, gapSize: 0.15, transparent: true, opacity: 0.6,
      });
      const bypass = new THREE.Line(geo, bypassMat);
      bypass.computeLineDistances();
      bypass.userData = { type: 'trace', path: 'cs-shortcircuit' };
      this.circuitGroup.add(bypass);
    }
  }

  _buildFaceConnectors() {
    const faces = this.engineData.faces;
    if (!faces) return;

    const kinds = faces.kinds || [];
    const total = kinds.length;
    const boardW = 14, boardH = 9;

    kinds.forEach((faceKind, i) => {
      // Distribute around board perimeter
      const perim = 2 * (boardW + boardH);
      const pos = (i / total) * perim;
      let x, y;
      if (pos < boardW) { x = -boardW + pos * 2; y = boardH; }
      else if (pos < boardW + boardH) { x = boardW; y = boardH - (pos - boardW) * 2; }
      else if (pos < 2 * boardW + boardH) { x = boardW - (pos - boardW - boardH) * 2; y = -boardH; }
      else { x = -boardW; y = -boardH + (pos - 2 * boardW - boardH) * 2; }

      // Connector shape based on kind
      let connGeo;
      switch (faceKind.shape) {
        case 'circle':   connGeo = new THREE.CircleGeometry(0.35, 16); break;
        case 'square':   connGeo = new THREE.PlaneGeometry(0.6, 0.6); break;
        case 'diamond':  connGeo = new THREE.CircleGeometry(0.35, 4); break;
        case 'hexagon':  connGeo = new THREE.CircleGeometry(0.35, 6); break;
        case 'triangle': connGeo = new THREE.CircleGeometry(0.35, 3); break;
        case 'octagon':  connGeo = new THREE.CircleGeometry(0.35, 8); break;
        default:         connGeo = new THREE.PlaneGeometry(0.7, 0.4); break;
      }

      const isLocal = faceKind.category === 'local';
      const connColor = isLocal ? 0x58a6ff : 0x3fb950;
      const connMat = new THREE.MeshBasicMaterial({ color: connColor });
      const conn = new THREE.Mesh(connGeo, connMat);
      conn.position.set(x, y, 0.1);
      conn.userData = { type: 'face_connector', faceKind, interactive: true };
      this.circuitGroup.add(conn);

      // Label
      const lbl = this._makeLabel(faceKind.id, 0.3, isLocal ? '#58a6ff' : '#3fb950');
      lbl.position.set(x, y - 0.5, 0.2);
      this.circuitGroup.add(lbl);
    });
  }

  _buildTableChips() {
    const tables = this.engineData.tables;
    if (!tables) return;

    // Position table chips alongside their related pipeline stages
    const tablePositions = {
      pit:            { x: 0,  y: -6 },
      fib:            { x: 5,  y: 5 },
      cs:             { x: -5, y: 6 },
      strategy_table: { x: 8,  y: 3 },
      measurements:   { x: 8,  y: -1 },
    };

    for (const [id, table] of Object.entries(tables)) {
      const pos = tablePositions[id];
      if (!pos) continue;

      const chipW = 3.5, chipH = 1.5;
      const chipGeo = new THREE.BoxGeometry(chipW, chipH, 0.15);
      const chipMat = new THREE.MeshPhongMaterial({
        color: 0x1a2332,
        emissive: 0x112233,
        emissiveIntensity: 0.15,
      });
      const chip = new THREE.Mesh(chipGeo, chipMat);
      chip.position.set(pos.x, pos.y, 0);
      chip.userData = { type: 'table_chip', tableId: id, table, interactive: true };
      this.circuitGroup.add(chip);
      this.chipMeshes.push(chip);

      const lbl = this._makeLabel(table.label, 0.42, '#d2a8ff');
      lbl.position.set(pos.x, pos.y + 0.25, 0.15);
      this.circuitGroup.add(lbl);

      const backingLbl = this._makeLabel(table.backing, 0.28, '#8b949e');
      backingLbl.position.set(pos.x, pos.y - 0.25, 0.15);
      this.circuitGroup.add(backingLbl);
    }
  }

  _buildSecurityComponents() {
    const sec = this.engineData.security;
    if (!sec) return;

    // Security subsystem sits in the upper-right quadrant of the board,
    // connected to the Validation stage chip via traces.
    const secBaseX = -2, secBaseY = -7;

    // ── SecurityProfile DIP switch ───────────────────────────────────────
    const profileData = sec.profiles || [];
    const dipW = 6, dipH = 1.2;
    const dipGeo = new THREE.BoxGeometry(dipW, dipH, 0.12);
    const dipMat = new THREE.MeshPhongMaterial({ color: 0x1a1a2e, emissive: 0x111122, emissiveIntensity: 0.1 });
    const dipSwitch = new THREE.Mesh(dipGeo, dipMat);
    dipSwitch.position.set(secBaseX, secBaseY, 0);
    dipSwitch.userData = {
      type: 'security_component', id: 'profiles',
      label: 'Security Profile (DIP Switch)',
      info: sec.profiles,
      design_note: 'Controls validation behavior: Disabled (NFD default), AcceptSigned (verify only), Default (full chain), Custom.',
      interactive: true,
    };
    this.circuitGroup.add(dipSwitch);
    this.chipMeshes.push(dipSwitch);

    // Individual DIP switch toggles for each profile
    profileData.forEach((profile, i) => {
      const toggleGeo = new THREE.BoxGeometry(0.5, 0.7, 0.15);
      const isActive = profile.name === 'Disabled'; // default state
      const toggleMat = new THREE.MeshBasicMaterial({ color: isActive ? 0x3fb950 : 0x30363d });
      const toggle = new THREE.Mesh(toggleGeo, toggleMat);
      toggle.position.set(secBaseX - dipW / 2 + 1 + i * 1.4, secBaseY, 0.1);
      this.circuitGroup.add(toggle);

      const toggleLabel = this._makeLabel(profile.name, 0.22, isActive ? '#3fb950' : '#8b949e');
      toggleLabel.position.set(secBaseX - dipW / 2 + 1 + i * 1.4, secBaseY - 0.8, 0.15);
      this.circuitGroup.add(toggleLabel);
    });

    const dipLabel = this._makeLabel('Security Profile', 0.35, '#d29922');
    dipLabel.position.set(secBaseX, secBaseY + 0.9, 0.15);
    this.circuitGroup.add(dipLabel);

    // ── KeyChain chip ────────────────────────────────────────────────────
    const kcX = -8, kcY = -7;
    this._buildSecurityChip(kcX, kcY, sec.keychain, 'key_generator', '#d29922');

    // ── Signers chip (crypto coprocessor) ────────────────────────────────
    const sigX = -8, sigY = -9.5;
    this._buildSecurityChip(sigX, sigY, sec.signers, 'crypto_chip', '#ff7b72');

    // ── Validator chip ───────────────────────────────────────────────────
    // This connects to the ValidationStage in the main pipeline
    const valX = -2, valY = -9.5;
    this._buildSecurityChip(valX, valY, sec.validator, 'verification_chip', '#d2a8ff');

    // ── CertCache chip ───────────────────────────────────────────────────
    const ccX = 3, ccY = -9.5;
    this._buildSecurityChip(ccX, ccY, sec.cert_cache, 'memory_chip', '#79c0ff');

    // ── SafeData output gate ─────────────────────────────────────────────
    const sdX = 3, sdY = -7;
    const sdGeo = new THREE.BoxGeometry(2.5, 1.2, 0.15);
    const sdMat = new THREE.MeshPhongMaterial({ color: 0x2d7a3a, emissive: 0x1a4d24, emissiveIntensity: 0.2 });
    const sdMesh = new THREE.Mesh(sdGeo, sdMat);
    sdMesh.position.set(sdX, sdY, 0);
    sdMesh.userData = {
      type: 'security_component', id: 'safe_data',
      label: 'SafeData',
      info: sec.safe_data,
      design_note: sec.safe_data?.design_note || '',
      interactive: true,
    };
    this.circuitGroup.add(sdMesh);
    this.chipMeshes.push(sdMesh);

    const sdLabel = this._makeLabel('SafeData ✓', 0.4, '#3fb950');
    sdLabel.position.set(sdX, sdY + 0.1, 0.15);
    this.circuitGroup.add(sdLabel);

    const sdSubLabel = this._makeLabel('pub(crate) — compiler proof', 0.25, '#8b949e');
    sdSubLabel.position.set(sdX, sdY - 0.35, 0.15);
    this.circuitGroup.add(sdSubLabel);

    // ── Traces connecting security components ────────────────────────────
    const secTraceMat = new THREE.LineBasicMaterial({ color: 0xd29922, transparent: true, opacity: 0.6 });

    // KeyChain → Signers
    this._addTrace(kcX + 1.75, kcY, sigX + 1.75, sigY + 0.6, secTraceMat);
    // Signers → Validator (crypto results flow to validation)
    this._addTrace(sigX + 1.75, sigY, valX - 1.75, valY, secTraceMat);
    // Validator → CertCache
    this._addTrace(valX + 1.75, valY, ccX - 1.25, ccY, secTraceMat);
    // Validator → SafeData (output)
    this._addTrace(valX + 1, valY + 0.6, sdX - 1.25, sdY, new THREE.LineBasicMaterial({ color: 0x3fb950, transparent: true, opacity: 0.6 }));

    // Validation stage → Validator (pipeline connection)
    const valStagePos = STAGE_POSITIONS.validation;
    if (valStagePos) {
      this._addTrace(valStagePos.x, valStagePos.y - 1, valX, valY + 0.6,
        new THREE.LineDashedMaterial({ color: 0xd2a8ff, dashSize: 0.2, gapSize: 0.1, transparent: true, opacity: 0.5 }));
    }
  }

  _buildSecurityChip(x, y, data, circuitType, color) {
    if (!data) return;
    const chipW = 3.5, chipH = 1.5;
    const chipGeo = new THREE.BoxGeometry(chipW, chipH, 0.15);
    const chipColor = new THREE.Color(color).multiplyScalar(0.3);
    const chipMat = new THREE.MeshPhongMaterial({
      color: chipColor,
      emissive: chipColor,
      emissiveIntensity: 0.15,
    });
    const chip = new THREE.Mesh(chipGeo, chipMat);
    chip.position.set(x, y, 0);
    chip.userData = {
      type: 'security_component', id: data.label?.toLowerCase().replace(/\s+/g, '_'),
      label: data.label,
      info: data,
      design_note: data.design_note || '',
      interactive: true,
    };
    this.circuitGroup.add(chip);
    this.chipMeshes.push(chip);

    const label = this._makeLabel(data.label, 0.38, color);
    label.position.set(x, y + 0.2, 0.15);
    this.circuitGroup.add(label);

    if (data.struct) {
      const structLabel = this._makeLabel(data.struct, 0.26, '#8b949e');
      structLabel.position.set(x, y - 0.25, 0.15);
      this.circuitGroup.add(structLabel);
    }
  }

  _addTrace(x1, y1, x2, y2, material) {
    const mid = { x: (x1 + x2) / 2, y: (y1 + y2) / 2 };
    const curve = new THREE.QuadraticBezierCurve3(
      new THREE.Vector3(x1, y1, 0.05),
      new THREE.Vector3(mid.x, mid.y + 0.3, 0.05),
      new THREE.Vector3(x2, y2, 0.05),
    );
    const points = curve.getPoints(16);
    const geo = new THREE.BufferGeometry().setFromPoints(points);
    const line = new THREE.Line(geo, material);
    if (material.isLineDashedMaterial) line.computeLineDistances();
    this.circuitGroup.add(line);
  }

  _buildPerformanceCallouts() {
    const perfData = this.engineData.performance;
    if (!perfData || perfData.length === 0) return;

    // Performance annotations are small indicators on the board that
    // glow when hovered, showing the specific performance insight.
    // They're positioned near the components they annotate.

    const annotationPositions = {
      ownership:    { x: -10, y: 1.8 },   // above decode (applies to all stages)
      zero_copy:    { x: -7,  y: 4.5 },   // above CS lookup
      smallvec:     { x: 2,   y: 1.8 },   // above PIT check / strategy
      dashmap:      { x: 0,   y: -4.5 },  // below PIT table
      handover:     { x: 7,   y: 6.5 },   // above FIB table
      decide_sync:  { x: 7,   y: 1.8 },   // above strategy
      batch_drain:  { x: -13, y: 0 },     // left edge (dispatcher)
      sign_sync:    { x: -6,  y: -8 },    // near signers chip
    };

    for (const perf of perfData) {
      const pos = annotationPositions[perf.id];
      if (!pos) continue;

      // Small diamond indicator
      const diamondGeo = new THREE.CircleGeometry(0.25, 4);
      const diamondMat = new THREE.MeshBasicMaterial({ color: 0xffa657, transparent: true, opacity: 0.8 });
      const diamond = new THREE.Mesh(diamondGeo, diamondMat);
      diamond.rotation.z = Math.PI / 4;
      diamond.position.set(pos.x, pos.y, 0.2);
      diamond.userData = {
        type: 'perf_callout',
        perf,
        interactive: true,
      };
      this.circuitGroup.add(diamond);
      this.chipMeshes.push(diamond);

      // Short annotation label
      const annoLabel = this._makeLabel(perf.annotation, 0.26, '#ffa657');
      annoLabel.position.set(pos.x, pos.y - 0.45, 0.2);
      this.circuitGroup.add(annoLabel);
    }
  }

  _buildPacketControls() {
    const toolbar = document.createElement('div');
    toolbar.className = 'arch-packet-controls';
    toolbar.style.display = 'none';
    toolbar.innerHTML = `
      <button class="arch-btn" data-packet="interest">Send Interest</button>
      <button class="arch-btn" data-packet="data">Send Data</button>
      <button class="arch-btn" data-packet="nack">Send Nack</button>
    `;
    this.container.appendChild(toolbar);
    this.packetControls = toolbar;

    toolbar.querySelectorAll('.arch-btn').forEach(btn => {
      btn.addEventListener('click', () => this._animatePacket(btn.dataset.packet));
    });

    // ── Bytes Buffer Strip (right side panel) ─────────────────────────────
    const bytesPanel = document.createElement('div');
    bytesPanel.className = 'arch-bytes-panel';
    bytesPanel.id = 'arch-bytes-panel';
    bytesPanel.style.display = 'none';
    this.container.appendChild(bytesPanel);

    // ── PacketContext Evolution Panel (left side panel) ────────────────────
    const ctxPanel = document.createElement('div');
    ctxPanel.className = 'arch-ctx-panel';
    ctxPanel.id = 'arch-ctx-panel';
    ctxPanel.style.display = 'none';
    this.container.appendChild(ctxPanel);
  }

  // ── Packet Animation (with Bytes strip + PacketContext evolution) ────────

  _animatePacket(type) {
    const path = type === 'interest' ? this.engineData.pipeline.interest_path
               : type === 'data'     ? this.engineData.pipeline.data_path
               : this.engineData.pipeline.nack_path;
    if (!path || path.length === 0) return;

    const color = type === 'interest' ? CIRCUIT_COLORS.interest
               : type === 'data'     ? CIRCUIT_COLORS.data
               : CIRCUIT_COLORS.nack;

    const bytesPanel = document.getElementById('arch-bytes-panel');
    const ctxPanel = document.getElementById('arch-ctx-panel');
    if (bytesPanel) { bytesPanel.style.display = 'block'; bytesPanel.innerHTML = ''; }
    if (ctxPanel) { ctxPanel.style.display = 'block'; ctxPanel.innerHTML = ''; }

    // Build the PacketContext field tracker
    const allFields = (this.engineData.context_fields || []);
    const ctxEvolution = type === 'interest'
      ? this.engineData.context_evolution?.interest_path
      : this.engineData.context_evolution?.data_path;

    if (ctxPanel) {
      ctxPanel.innerHTML = `
        <div class="ctx-panel-title">PacketContext <span class="ctx-ownership">by value (move)</span></div>
        <div class="ctx-fields-grid" id="ctx-fields-grid">
          ${allFields.map(f => `
            <div class="ctx-field-row" id="ctx-f-${f.name}" data-field="${f.name}">
              <span class="ctx-fname">${f.name}</span>
              <span class="ctx-ftype">${f.type}</span>
              <div class="ctx-fbar"></div>
            </div>
          `).join('')}
        </div>
        <div class="ctx-stage-note" id="ctx-stage-note"></div>
      `;
    }

    // Build the Bytes buffer visualization
    if (bytesPanel) {
      bytesPanel.innerHTML = `
        <div class="bytes-title">bytes::Bytes <span class="bytes-rc" id="bytes-rc">rc=1</span></div>
        <div class="bytes-buffer" id="bytes-buffer">
          <div class="bytes-segment bytes-header" style="width:8%">LP</div>
          <div class="bytes-segment bytes-type" style="width:4%">T</div>
          <div class="bytes-segment bytes-name" style="width:35%">/ndn/edu/ucla/cs</div>
          <div class="bytes-segment bytes-nonce" style="width:10%">Nonce</div>
          <div class="bytes-segment bytes-lifetime" style="width:8%">LT</div>
          <div class="bytes-segment bytes-sig" style="width:20%">SigInfo</div>
          <div class="bytes-segment bytes-content" style="width:15%">Content</div>
        </div>
        <div class="bytes-slices" id="bytes-slices"></div>
        <div class="bytes-ops-log" id="bytes-ops-log"></div>
      `;
    }

    // Create particle
    const particleGeo = new THREE.SphereGeometry(0.2, 12, 8);
    const particleMat = new THREE.MeshBasicMaterial({ color });
    const particle = new THREE.Mesh(particleGeo, particleMat);
    this.circuitGroup.add(particle);

    const glowGeo = new THREE.SphereGeometry(0.35, 12, 8);
    const glowMat = new THREE.MeshBasicMaterial({ color, transparent: true, opacity: 0.3 });
    particle.add(new THREE.Mesh(glowGeo, glowMat));

    const stagePositions = path.map(id => STAGE_POSITIONS[id]).filter(Boolean);
    if (stagePositions.length === 0) return;

    let stageIndex = 0;
    let t = 0;
    const speed = 0.015;
    let cumulativeNs = 0;

    // Mark initial context fields
    this._updateContextPanel('inbound', ctxEvolution, 0);
    this._updateBytesStrip('socket', 1, bytesPanel);

    const step = () => {
      if (stageIndex >= stagePositions.length - 1) {
        this.circuitGroup.remove(particle);
        // Final state
        setTimeout(() => {
          if (bytesPanel) bytesPanel.style.display = 'none';
          if (ctxPanel) ctxPanel.style.display = 'none';
        }, 3000);
        return;
      }

      t += speed;
      const from = stagePositions[stageIndex];
      const to = stagePositions[stageIndex + 1];
      particle.position.x = from.x + (to.x - from.x) * t;
      particle.position.y = from.y + (to.y - from.y) * t;
      particle.position.z = 0.3;

      // Light up active trace
      this.traceLines.forEach(line => {
        if (line.userData.from === path[stageIndex] && line.userData.to === path[stageIndex + 1]) {
          line.material.color.setHex(CIRCUIT_COLORS.trace_glow);
          line.material.opacity = 1;
        }
      });

      if (t >= 1) {
        t = 0;
        stageIndex++;

        const currentStageId = path[stageIndex];
        const stageData = currentStageId && this.engineData.pipeline.stages[currentStageId];

        if (stageData) {
          // ── Update timescale accumulator ──────────────────────────
          const ts = stageData.timescale;
          if (ts) {
            const primaryTime = ts.interest_4comp || ts.hit || ts.new_entry ||
              ts.fib_lpm_100 || ts.disabled_passthrough || ts.cold_insert ||
              Object.values(ts)[0];
            cumulativeNs += parseTimescale(primaryTime);
          }

          // ── Update PacketContext fields ──────────────────────────
          this._updateContextPanel(currentStageId, ctxEvolution, cumulativeNs);

          // ── Update Bytes buffer strip ────────────────────────────
          const bytesOps = stageData.bytes_ops || [];
          if (bytesOps.length > 0) {
            const lastOp = bytesOps[bytesOps.length - 1];
            const rcMatch = lastOp.effect.match(/(\d+)[→+]/);
            const newRc = lastOp.effect.includes('+1')
              ? 'rc+1'
              : lastOp.effect;
            this._updateBytesStrip(currentStageId, newRc, bytesPanel, bytesOps);
          }

          // Highlight Name segments on decode
          if (currentStageId === 'decode') {
            this._highlightBytesSegment('bytes-name', true);
            this._highlightBytesSegment('bytes-type', true);
            this._addBytesSlice('Name', '35%', '8%', '#d2a8ff', 'Bytes::slice() — zero-copy view');
          }
          if (currentStageId === 'cs_insert') {
            this._addBytesSlice('CS copy', '100%', '0%', '#3fb950', 'raw_bytes.clone() — ref-count +1, same allocation');
          }
        }

        // Reset previous traces
        this.traceLines.forEach(line => {
          line.material.color.setHex(CIRCUIT_COLORS.trace);
          line.material.opacity = 0.8;
        });
      }

      requestAnimationFrame(step);
    };

    particle.position.set(stagePositions[0].x, stagePositions[0].y, 0.3);
    step();
  }

  _updateContextPanel(stageId, evolution, cumulativeNs) {
    if (!evolution) return;
    const stageInfo = evolution.find(s => s.stage === stageId);
    if (!stageInfo) return;

    // Light up newly set fields
    for (const field of stageInfo.fields_set) {
      const el = document.getElementById(`ctx-f-${field}`);
      if (el) {
        el.classList.add('ctx-active');
        const bar = el.querySelector('.ctx-fbar');
        if (bar) bar.style.width = '100%';
      }
    }

    // Update stage note with timing
    const noteEl = document.getElementById('ctx-stage-note');
    if (noteEl) {
      const timeStr = cumulativeNs > 0 ? ` <span class="ctx-time">${formatNs(cumulativeNs)}</span>` : '';
      noteEl.innerHTML = `${stageInfo.note}${timeStr}`;
    }
  }

  _updateBytesStrip(stageId, rcLabel, panel, ops) {
    const rcEl = document.getElementById('bytes-rc');
    if (rcEl && rcLabel) rcEl.textContent = typeof rcLabel === 'number' ? `rc=${rcLabel}` : rcLabel;

    // Append operation to log
    if (ops && ops.length > 0) {
      const logEl = document.getElementById('bytes-ops-log');
      if (logEl) {
        for (const op of ops) {
          const entry = document.createElement('div');
          entry.className = 'bytes-op-entry';
          entry.innerHTML = `
            <code>${op.op}</code>
            <span class="bytes-op-effect">${op.effect}</span>
            <span class="bytes-op-cost">${op.cost}</span>
          `;
          logEl.appendChild(entry);
          // Scroll to bottom
          logEl.scrollTop = logEl.scrollHeight;
        }
      }
    }
  }

  _highlightBytesSegment(className, active) {
    const el = document.querySelector(`.${className}`);
    if (el) el.classList.toggle('bytes-active', active);
  }

  _addBytesSlice(label, width, left, color, note) {
    const container = document.getElementById('bytes-slices');
    if (!container) return;
    const slice = document.createElement('div');
    slice.className = 'bytes-slice-indicator';
    slice.innerHTML = `<span class="bytes-slice-label" style="color:${color}">${label}</span><span class="bytes-slice-note">${note}</span>`;
    slice.style.borderColor = color;
    container.appendChild(slice);
  }

  // ── Level Transitions ────────────────────────────────────────────────────

  _setLevel(newLevel) {
    if (newLevel === this.level) return;
    this.level = newLevel;

    const isGalaxy = newLevel === 1;
    const isCircuit = newLevel === 2;

    this.galaxyGroup.visible = isGalaxy;
    this.circuitGroup.visible = isCircuit;
    if (this.packetControls) this.packetControls.style.display = isCircuit ? 'flex' : 'none';

    // Camera transition
    const target = isGalaxy ? new THREE.Vector3(0, 0, 40) : new THREE.Vector3(0, 0, 22);
    this._animateCamera(target);

    // Update toolbar buttons
    this.container.querySelectorAll('.arch-level-btn').forEach(btn => {
      btn.classList.toggle('active', parseInt(btn.dataset.level) === newLevel);
    });

    // Hide popup
    if (this.popupEl) this.popupEl.style.display = 'none';
  }

  _animateCamera(targetPos) {
    const start = this.camera.position.clone();
    const duration = 800;
    const startTime = performance.now();

    const animate = () => {
      const elapsed = performance.now() - startTime;
      const t = Math.min(elapsed / duration, 1);
      const ease = t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2; // ease in-out

      this.camera.position.lerpVectors(start, targetPos, ease);
      this.controls.target.set(0, 0, 0);

      if (t < 1) requestAnimationFrame(animate);
    };
    animate();
  }

  // ── Interaction ──────────────────────────────────────────────────────────

  _onMouseMove(e) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    this.mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;

    this.raycaster.setFromCamera(this.mouse, this.camera);
    const meshes = this.level === 1 ? this.crateNodes : this.chipMeshes;
    const intersects = this.raycaster.intersectObjects(meshes);

    if (intersects.length > 0) {
      const obj = intersects[0].object;
      this.renderer.domElement.style.cursor = 'pointer';

      // Tooltip
      const data = obj.userData;
      let html = '';
      if (data.type === 'crate') {
        html = `<strong>${data.crate.name}</strong><br>${data.crate.description}`;
      } else if (data.type === 'chip') {
        html = `<strong>${data.stage.chip_label}</strong><br>${data.stage.design_note || ''}`;
      } else if (data.type === 'table_chip') {
        html = `<strong>${data.table.label}</strong><br>${data.table.backing}<br>${data.table.design_note || ''}`;
      } else if (data.type === 'face_connector') {
        html = `<strong>${data.faceKind.id}</strong><br>${data.faceKind.description}`;
      } else if (data.type === 'security_component') {
        html = `<strong>${data.label}</strong><br>${data.design_note || ''}`;
      } else if (data.type === 'perf_callout') {
        const p = data.perf;
        html = `<strong>${p.icon} ${p.label}</strong><br>${p.what}<br><br><em>Why:</em> ${p.why}<br><code>${p.source}</code>`;
      } else if (data.type === 'task_box') {
        const t = data.task;
        html = `<strong>${data.label}</strong>`;
        if (t) {
          html += `<br><em>${t.kind}</em>`;
          if (t.period) html += ` · ${t.period}`;
          html += `<br>${t.design_note || ''}`;
          if (t.touches) html += `<br><strong>Touches:</strong> ${t.touches.join(', ')}`;
        }
      }

      if (html && this.tooltipEl) {
        this.tooltipEl.innerHTML = html;
        this.tooltipEl.style.display = 'block';
        this.tooltipEl.style.left = (e.clientX - rect.left + 15) + 'px';
        this.tooltipEl.style.top = (e.clientY - rect.top - 10) + 'px';
      }

      // Highlight
      if (this.hoveredObject && this.hoveredObject !== obj) {
        this._unhighlight(this.hoveredObject);
      }
      this._highlight(obj);
      this.hoveredObject = obj;
    } else {
      this.renderer.domElement.style.cursor = 'default';
      if (this.tooltipEl) this.tooltipEl.style.display = 'none';
      if (this.hoveredObject) {
        this._unhighlight(this.hoveredObject);
        this.hoveredObject = null;
      }
    }
  }

  _onClick(e) {
    const rect = this.renderer.domElement.getBoundingClientRect();
    this.mouse.x = ((e.clientX - rect.left) / rect.width) * 2 - 1;
    this.mouse.y = -((e.clientY - rect.top) / rect.height) * 2 + 1;

    this.raycaster.setFromCamera(this.mouse, this.camera);
    const meshes = this.level === 1 ? this.crateNodes : this.chipMeshes;
    const intersects = this.raycaster.intersectObjects(meshes);

    if (intersects.length > 0) {
      const data = intersects[0].object.userData;

      if (this.level === 1 && data.type === 'crate') {
        if (data.crate.name === 'ndn-engine') {
          // Zoom into engine circuit board
          this._setLevel(2);
        } else {
          // Show crate detail popup
          this._showCratePopup(data.crate, e, rect);
        }
      } else if (this.level === 2 && (data.type === 'chip' || data.type === 'table_chip')) {
        this._showChipPopup(data, e, rect);
      } else if (this.level === 2 && data.type === 'security_component') {
        this._showSecurityPopup(data, e, rect);
      } else if (this.level === 2 && data.type === 'perf_callout') {
        this._showPerfPopup(data.perf, e, rect);
      }
    } else {
      if (this.popupEl) this.popupEl.style.display = 'none';
    }
  }

  _showCratePopup(crate, e, rect) {
    if (!this.popupEl) return;
    const layer = this.app.data.layers.find(l => l.id === crate.layer);
    this.popupEl.innerHTML = `
      <div class="arch-popup-header">
        <strong>${crate.name}</strong>
        <button class="arch-popup-close" onclick="this.parentElement.parentElement.style.display='none'">&times;</button>
      </div>
      <p>${crate.description}</p>
      <div class="arch-popup-meta">
        <span class="badge">${layer?.label || crate.layer}</span>
        ${crate.no_std ? '<span class="badge badge-green">no_std</span>' : ''}
        <span class="badge">${crate.key_types.length} types</span>
      </div>
      <div class="arch-popup-types">${crate.key_types.slice(0, 8).map(t =>
        `<code>${t}</code>`).join(' ')}</div>
      <div class="arch-popup-links">
        <a href="#" onclick="event.preventDefault();" class="arch-link" data-action="show-crate" data-name="${crate.name}">Crate detail ↗</a>
      </div>
    `;
    this.popupEl.style.display = 'block';
    this.popupEl.style.left = Math.min(e.clientX - rect.left + 15, rect.width - 320) + 'px';
    this.popupEl.style.top = Math.min(e.clientY - rect.top - 10, rect.height - 300) + 'px';

    // Wire up crate detail link
    this.popupEl.querySelector('[data-action="show-crate"]')?.addEventListener('click', () => {
      this.app.showCrate(crate.name);
    });
  }

  _showChipPopup(data, e, rect) {
    if (!this.popupEl) return;

    const isStage = data.type === 'chip';
    const info = isStage ? data.stage : data.table;
    const label = isStage ? data.stage.chip_label : data.table.label;

    const fields = isStage
      ? `<div class="arch-chip-fields">
          <div><strong>Reads:</strong> ${(info.reads || []).map(f => `<code>${f}</code>`).join(', ') || 'none'}</div>
          <div><strong>Writes:</strong> ${(info.writes || []).map(f => `<code>${f}</code>`).join(', ') || 'none'}</div>
          <div><strong>Signature:</strong> <code>${info.signature || ''}</code></div>
        </div>`
      : `<div class="arch-chip-fields">
          <div><strong>Backing:</strong> <code>${info.backing || ''}</code></div>
          ${(info.fields || []).map(f => `<div><code>${f.name}: ${f.type}</code> — ${f.description}</div>`).join('')}
        </div>`;

    const shortCircuits = (info.short_circuits || []).map(sc =>
      `<div class="arch-sc"><span class="arch-sc-action">${sc.action}</span> ${sc.description}</div>`
    ).join('');

    this.popupEl.innerHTML = `
      <div class="arch-popup-header">
        <strong>${label}</strong>
        <button class="arch-popup-close" onclick="this.parentElement.parentElement.style.display='none'">&times;</button>
      </div>
      <div class="arch-design-note">${info.design_note || ''}</div>
      ${fields}
      ${shortCircuits ? `<div class="arch-short-circuits"><strong>Short-circuits:</strong>${shortCircuits}</div>` : ''}
      <div class="arch-popup-links">
        ${info.wiki_link ? `<a href="../wiki/${info.wiki_link}" target="_blank" rel="noopener" class="arch-link">Wiki ↗</a>` : ''}
        ${info.source ? `<a href="https://github.com/Quarmire/ndn-rs/blob/main/${info.source}" target="_blank" rel="noopener" class="arch-link">Source ↗</a>` : ''}
        <a href="#" class="arch-link arch-deep-dive-btn" data-graph="${this._findTypeGraph(data)}">Deep Dive ↗</a>
      </div>
    `;
    this.popupEl.style.display = 'block';
    this.popupEl.style.left = Math.min(e.clientX - rect.left + 15, rect.width - 350) + 'px';
    this.popupEl.style.top = Math.min(e.clientY - rect.top - 10, rect.height - 400) + 'px';

    // Wire up deep dive button
    const ddBtn = this.popupEl.querySelector('.arch-deep-dive-btn');
    if (ddBtn) {
      ddBtn.addEventListener('click', (ev) => {
        ev.preventDefault();
        const graphId = ddBtn.dataset.graph;
        if (graphId) this._showTypeGraph(graphId);
      });
    }
  }

  /** Map a chip click to the corresponding type_graph id. */
  _findTypeGraph(data) {
    if (data.type === 'chip') {
      const id = data.stageId;
      if (['decode', 'cs_lookup', 'pit_check', 'strategy', 'validation', 'pit_match', 'cs_insert'].includes(id))
        return 'pipeline_stage';
    }
    if (data.type === 'table_chip') {
      const id = data.tableId;
      if (id === 'cs') return 'content_store';
    }
    if (data.type === 'security_component') {
      return 'security_chain';
    }
    return '';
  }

  _showSecurityPopup(data, e, rect) {
    if (!this.popupEl) return;
    const info = data.info || {};

    let body = `<div class="arch-design-note">${data.design_note}</div>`;

    // Fields
    if (info.fields) {
      body += `<div class="arch-chip-fields">${info.fields.map(f =>
        `<div><code>${f.visibility ? f.visibility + ' ' : ''}${f.name}: ${f.type}</code> — ${f.description}</div>`
      ).join('')}</div>`;
    }

    // Signer implementations
    if (info.implementations) {
      body += `<div class="arch-chip-fields"><strong>Implementations:</strong>${info.implementations.map(impl =>
        `<div><code>${impl.name}</code> ${impl.sig_size}B, ${impl.speed} — ${impl.design_note}</div>`
      ).join('')}</div>`;
    }

    // Trust paths (SafeData)
    if (info.trust_paths) {
      body += `<div class="arch-chip-fields"><strong>Trust paths:</strong>${info.trust_paths.map(tp =>
        `<div><code>${tp.variant}</code> — ${tp.description}</div>`
      ).join('')}</div>`;
    }

    // Construction points (SafeData)
    if (info.construction_points) {
      body += `<div class="arch-chip-fields"><strong>Can only be constructed at:</strong>${info.construction_points.map(cp =>
        `<div><code>${cp.location}</code> — ${cp.method}</div>`
      ).join('')}</div>`;
    }

    // Chain walk (Validator)
    if (info.chain_walk) {
      body += `<div class="arch-chip-fields"><strong>Chain walk:</strong>${info.chain_walk.map(step =>
        `<div>${step}</div>`
      ).join('')}</div>`;
    }

    // Constructors (KeyChain)
    if (info.constructors) {
      body += `<div class="arch-chip-fields"><strong>Constructors:</strong>${info.constructors.map(c =>
        `<div><code>${c.name}</code> — ${c.description}</div>`
      ).join('')}</div>`;
    }

    // DIP switch profiles
    if (Array.isArray(info)) {
      body += `<table class="arch-profile-table">
        <tr><th>Profile</th><th>Schema</th><th>Verify</th><th>Chain</th><th>Use case</th></tr>
        ${info.map(p => `<tr>
          <td><strong>${p.name}</strong></td>
          <td>${p.schema}</td>
          <td>${p.sig_verify ? '✓' : '—'}</td>
          <td>${p.chain_fetch === true ? '✓' : p.chain_fetch === false ? '—' : p.chain_fetch}</td>
          <td>${p.use_case}</td>
        </tr>`).join('')}
      </table>`;
    }

    // Links
    const source = info.source || info.trait_source;
    body += `<div class="arch-popup-links">
      ${source ? `<a href="https://github.com/Quarmire/ndn-rs/blob/main/${source.split(':')[0]}" target="_blank" rel="noopener" class="arch-link">Source ↗</a>` : ''}
    </div>`;

    this.popupEl.innerHTML = `
      <div class="arch-popup-header">
        <strong>${data.label}</strong>
        <button class="arch-popup-close" onclick="this.parentElement.parentElement.style.display='none'">&times;</button>
      </div>
      ${body}
    `;
    this.popupEl.style.display = 'block';
    this.popupEl.style.left = Math.min(e.clientX - rect.left + 15, rect.width - 380) + 'px';
    this.popupEl.style.top = Math.min(e.clientY - rect.top - 10, rect.height - 400) + 'px';
  }

  _showPerfPopup(perf, e, rect) {
    if (!this.popupEl) return;

    const appliesTo = (perf.applies_to || []).map(id => `<code>${id}</code>`).join(', ');

    this.popupEl.innerHTML = `
      <div class="arch-popup-header">
        <strong>${perf.icon} ${perf.label}</strong>
        <button class="arch-popup-close" onclick="this.parentElement.parentElement.style.display='none'">&times;</button>
      </div>
      <div class="arch-perf-what">${perf.what}</div>
      <div class="arch-perf-why"><strong>Why this matters:</strong> ${perf.why}</div>
      <div class="arch-perf-source"><code>${perf.source}</code></div>
      ${appliesTo ? `<div class="arch-perf-applies"><strong>Applies to:</strong> ${appliesTo}</div>` : ''}
      <div class="arch-popup-links">
        <a href="https://github.com/Quarmire/ndn-rs/blob/main/${perf.source.split(':')[0]}" target="_blank" rel="noopener" class="arch-link">Source ↗</a>
      </div>
    `;
    this.popupEl.style.display = 'block';
    this.popupEl.style.left = Math.min(e.clientX - rect.left + 15, rect.width - 380) + 'px';
    this.popupEl.style.top = Math.min(e.clientY - rect.top - 10, rect.height - 350) + 'px';
  }

  _highlight(obj) {
    if (obj.material && obj.material.emissiveIntensity !== undefined) {
      obj._savedEmissive = obj.material.emissiveIntensity;
      obj.material.emissiveIntensity = 0.6;
    }
  }

  _unhighlight(obj) {
    if (obj.material && obj._savedEmissive !== undefined) {
      obj.material.emissiveIntensity = obj._savedEmissive;
    }
  }

  // ── Level 3: Type Graph (Node-Graph / Shader-Graph View) ─────────────────

  _showTypeGraph(graphId) {
    const graphs = this.engineData?.type_graphs;
    if (!graphs || !graphs[graphId]) return;

    const graph = graphs[graphId];
    this.popupEl.style.display = 'none'; // close chip popup

    // Create or reuse the full-screen type graph overlay
    let overlay = document.getElementById('type-graph-overlay');
    if (!overlay) {
      overlay = document.createElement('div');
      overlay.id = 'type-graph-overlay';
      overlay.className = 'type-graph-overlay';
      this.container.appendChild(overlay);
    }
    overlay.style.display = 'flex';

    // Build the node graph using HTML/CSS (not Three.js — 2D is better for readable text)
    const SCALE = 70; // pixels per graph unit
    const PAD = 40;

    // Calculate bounds
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity;
    for (const n of graph.nodes) {
      minX = Math.min(minX, n.x); minY = Math.min(minY, n.y);
      maxX = Math.max(maxX, n.x); maxY = Math.max(maxY, n.y);
    }
    const width = (maxX - minX) * SCALE + 400;
    const height = (maxY - minY) * SCALE + 300;

    const kindColors = {
      trait: { bg: '#2d1a50', border: '#d2a8ff', text: '#d2a8ff' },
      struct: { bg: '#1a2d50', border: '#58a6ff', text: '#58a6ff' },
      enum: { bg: '#1a3322', border: '#3fb950', text: '#3fb950' },
      impl: { bg: '#1a1a2e', border: '#8b949e', text: '#8b949e' },
    };

    // Build SVG for edges
    let svgEdges = '';
    for (const edge of graph.edges) {
      const fromNode = graph.nodes.find(n => n.id === edge.from);
      const toNode = graph.nodes.find(n => n.id === edge.to);
      if (!fromNode || !toNode) continue;

      const x1 = (fromNode.x - minX) * SCALE + PAD + 90;
      const y1 = (fromNode.y - minY) * SCALE + PAD + 30;
      const x2 = (toNode.x - minX) * SCALE + PAD + 90;
      const y2 = (toNode.y - minY) * SCALE + PAD + 30;
      const mx = (x1 + x2) / 2;
      const my = (y1 + y2) / 2 - 15;

      const dashStyle = edge.style === 'dashed' ? 'stroke-dasharray="6,4"'
                       : edge.style === 'dotted' ? 'stroke-dasharray="2,3"' : '';
      const color = edge.style === 'solid' ? '#58a6ff' : '#8b949e';

      svgEdges += `<path d="M${x1},${y1} Q${mx},${my} ${x2},${y2}" stroke="${color}" stroke-width="1.5" fill="none" ${dashStyle} marker-end="url(#arrow)"/>`;

      // Edge label at midpoint
      if (edge.label) {
        svgEdges += `<text x="${mx}" y="${my - 6}" font-size="10" fill="#8b949e" text-anchor="middle" font-family="monospace">${edge.label}</text>`;
      }
    }

    // Build node HTML
    let nodeHtml = '';
    for (const node of graph.nodes) {
      const x = (node.x - minX) * SCALE + PAD;
      const y = (node.y - minY) * SCALE + PAD;
      const colors = kindColors[node.kind] || kindColors.struct;
      const kindLabel = node.kind.charAt(0).toUpperCase() + node.kind.slice(1);

      let slots = '';
      if (node.methods) {
        slots += node.methods.map(m => `<div class="tg-slot tg-method">${m}</div>`).join('');
      }
      if (node.fields) {
        slots += node.fields.map(f => `<div class="tg-slot tg-field">${f}</div>`).join('');
      }
      if (node.variants) {
        slots += node.variants.map(v => `<div class="tg-slot tg-variant">${v}</div>`).join('');
      }
      if (node.implements) {
        slots += `<div class="tg-slot tg-impl">impl ${node.implements}</div>`;
      }

      nodeHtml += `
        <div class="tg-node" style="left:${x}px;top:${y}px;border-color:${colors.border};background:${colors.bg};">
          <div class="tg-node-header" style="color:${colors.text};">
            <span class="tg-kind">${kindLabel}</span>
            <span class="tg-name">${node.label}</span>
          </div>
          ${slots ? `<div class="tg-slots">${slots}</div>` : ''}
          ${node.source ? `<a class="tg-source" href="https://github.com/Quarmire/ndn-rs/blob/main/crates/engine/${node.source.split(':')[0]}" target="_blank" rel="noopener">${node.source}</a>` : ''}
        </div>
      `;
    }

    overlay.innerHTML = `
      <div class="tg-header">
        <strong>${graph.title}</strong>
        <button class="tg-close" onclick="document.getElementById('type-graph-overlay').style.display='none'">&times; Back to Circuit</button>
      </div>
      <div class="tg-canvas" style="width:${width}px;height:${height}px;position:relative;">
        <svg style="position:absolute;top:0;left:0;width:100%;height:100%;pointer-events:none;">
          <defs>
            <marker id="arrow" markerWidth="8" markerHeight="6" refX="8" refY="3" orient="auto">
              <path d="M0,0 L8,3 L0,6" fill="#58a6ff" />
            </marker>
          </defs>
          ${svgEdges}
        </svg>
        ${nodeHtml}
      </div>
    `;
  }

  // ── Toolbar ──────────────────────────────────────────────────────────────

  _buildToolbar() {
    const toolbar = document.createElement('div');
    toolbar.className = 'arch-toolbar';
    toolbar.innerHTML = `
      <button class="arch-level-btn active" data-level="1">Galaxy</button>
      <button class="arch-level-btn" data-level="2">Engine</button>
      <span class="arch-toolbar-sep">|</span>
      <button class="arch-overlay-btn" data-overlay="tasks" title="Show Tokio task topology">Tasks</button>
    `;
    this.container.appendChild(toolbar);

    toolbar.querySelectorAll('.arch-level-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        this._setLevel(parseInt(btn.dataset.level));
      });
    });

    toolbar.querySelectorAll('.arch-overlay-btn').forEach(btn => {
      btn.addEventListener('click', () => {
        btn.classList.toggle('active');
        this._toggleTaskOverlay(btn.classList.contains('active'));
      });
    });
  }

  // ── Task Topology Overlay ───────────────────────────────────────────────

  _toggleTaskOverlay(visible) {
    if (!this.taskOverlay) this._buildTaskOverlay();
    if (this.taskOverlay) this.taskOverlay.style.display = visible ? 'block' : 'none';
    if (this.taskGroup) this.taskGroup.visible = visible;
  }

  _buildTaskOverlay() {
    const tasks = this.engineData?.tasks;
    if (!tasks) return;

    // 3D overlay group on the circuit board
    this.taskGroup = new THREE.Group();
    this.taskGroup.name = 'tasks';
    this.taskGroup.visible = false;
    this.circuitGroup.add(this.taskGroup);

    // Darken the board slightly when tasks are shown
    const overlayGeo = new THREE.PlaneGeometry(30, 20);
    const overlayMat = new THREE.MeshBasicMaterial({
      color: 0x000000, transparent: true, opacity: 0.4, depthWrite: false,
    });
    const overlay = new THREE.Mesh(overlayGeo, overlayMat);
    overlay.position.z = 0.3;
    this.taskGroup.add(overlay);

    // ── Main pipeline bus (the central channel) ──────────────────────
    const busMat = new THREE.MeshBasicMaterial({ color: 0xc87533, transparent: true, opacity: 0.7 });
    const busGeo = new THREE.PlaneGeometry(20, 0.4);
    const bus = new THREE.Mesh(busGeo, busMat);
    bus.position.set(0, 0, 0.35);
    this.taskGroup.add(bus);

    const busLabel = this._makeLabel('mpsc<InboundPacket> cap=4096', 0.26, '#c87533');
    busLabel.position.set(0, 0.45, 0.4);
    this.taskGroup.add(busLabel);

    // ── Face Reader lanes (top, feeding into bus) ────────────────────
    const readerY = 4;
    const faceCount = 5; // representative
    for (let i = 0; i < faceCount; i++) {
      const x = -8 + i * 4;
      this._drawTaskLane(x, readerY, x, 0.3, 0x58a6ff, 0.35);

      // Reader task box
      const boxGeo = new THREE.PlaneGeometry(2.2, 0.8);
      const boxMat = new THREE.MeshBasicMaterial({ color: 0x1a3050, transparent: true, opacity: 0.9 });
      const box = new THREE.Mesh(boxGeo, boxMat);
      box.position.set(x, readerY + 0.5, 0.35);
      box.userData = {
        type: 'task_box',
        task: tasks.per_face[0],
        label: `Face Reader ${i}`,
        interactive: true,
      };
      this.taskGroup.add(box);
      this.chipMeshes.push(box);

      const lbl = this._makeLabel(`reader[${i}]`, 0.24, '#58a6ff');
      lbl.position.set(x, readerY + 0.5, 0.4);
      this.taskGroup.add(lbl);

      // IO icon (small pulse)
      const pulseGeo = new THREE.CircleGeometry(0.12, 8);
      const pulseMat = new THREE.MeshBasicMaterial({ color: 0x58a6ff });
      const pulse = new THREE.Mesh(pulseGeo, pulseMat);
      pulse.position.set(x - 0.9, readerY + 0.5, 0.4);
      this.taskGroup.add(pulse);
    }

    const readersLabel = this._makeLabel('Face Reader Tasks (1 per face, IO-bound)', 0.28, '#58a6ff');
    readersLabel.position.set(0, readerY + 1.5, 0.4);
    this.taskGroup.add(readersLabel);

    // ── Pipeline Runner (center) ─────────────────────────────────────
    const prGeo = new THREE.PlaneGeometry(4, 1.2);
    const prMat = new THREE.MeshBasicMaterial({ color: 0x2d1a50, transparent: true, opacity: 0.9 });
    const pr = new THREE.Mesh(prGeo, prMat);
    pr.position.set(0, 0, 0.4);
    pr.userData = {
      type: 'task_box',
      task: tasks.permanent[0],
      label: 'Pipeline Runner',
      interactive: true,
    };
    this.taskGroup.add(pr);
    this.chipMeshes.push(pr);

    const prLabel = this._makeLabel('Pipeline Runner', 0.32, '#d2a8ff');
    prLabel.position.set(0, 0.2, 0.45);
    this.taskGroup.add(prLabel);
    const prSub = this._makeLabel('batch drain → fragment sieve → dispatch', 0.2, '#8b949e');
    prSub.position.set(0, -0.2, 0.45);
    this.taskGroup.add(prSub);

    // ── Per-packet task spawns (fanning out below pipeline runner) ────
    const spawnY = -1.5;
    for (let i = 0; i < 4; i++) {
      const x = -3 + i * 2;
      this._drawTaskLane(0, -0.6, x, spawnY, 0xd2a8ff, 0.2);

      const taskGeo = new THREE.CircleGeometry(0.3, 8);
      const taskMat = new THREE.MeshBasicMaterial({ color: 0xd2a8ff, transparent: true, opacity: 0.7 });
      const task = new THREE.Mesh(taskGeo, taskMat);
      task.position.set(x, spawnY, 0.35);
      this.taskGroup.add(task);
    }

    const spawnLabel = this._makeLabel('Per-Packet Tasks (if pipeline_threads > 1)', 0.24, '#d2a8ff');
    spawnLabel.position.set(0, spawnY - 0.6, 0.4);
    this.taskGroup.add(spawnLabel);

    // ── Face Sender lanes (bottom, reading from per-face queues) ──────
    const senderY = -4;
    for (let i = 0; i < faceCount; i++) {
      const x = -8 + i * 4;
      this._drawTaskLane(x, -2.5, x, senderY, 0x3fb950, 0.35);

      const boxGeo = new THREE.PlaneGeometry(2.2, 0.8);
      const boxMat = new THREE.MeshBasicMaterial({ color: 0x1a3322, transparent: true, opacity: 0.9 });
      const box = new THREE.Mesh(boxGeo, boxMat);
      box.position.set(x, senderY - 0.5, 0.35);
      box.userData = {
        type: 'task_box',
        task: tasks.per_face[1],
        label: `Face Sender ${i}`,
        interactive: true,
      };
      this.taskGroup.add(box);
      this.chipMeshes.push(box);

      const lbl = this._makeLabel(`sender[${i}]`, 0.24, '#3fb950');
      lbl.position.set(x, senderY - 0.5, 0.4);
      this.taskGroup.add(lbl);

      // Queue capacity label
      const qLbl = this._makeLabel('cap=2048', 0.18, '#8b949e');
      qLbl.position.set(x, senderY + 0.2, 0.4);
      this.taskGroup.add(qLbl);
    }

    const sendersLabel = this._makeLabel('Face Sender Tasks (1 per face, IO-bound, preserves ordering)', 0.28, '#3fb950');
    sendersLabel.position.set(0, senderY - 1.5, 0.4);
    this.taskGroup.add(sendersLabel);

    // ── Timer tasks (right side, as oscillator circuits) ─────────────
    const timerTasks = tasks.permanent.filter(t => t.kind === 'timer');
    const timerBaseX = 11, timerBaseY = 3;

    timerTasks.forEach((task, i) => {
      const y = timerBaseY - i * 1.5;

      // Oscillator symbol (small square wave)
      const oscGeo = new THREE.PlaneGeometry(2.5, 0.7);
      const oscMat = new THREE.MeshBasicMaterial({ color: 0x332211, transparent: true, opacity: 0.85 });
      const osc = new THREE.Mesh(oscGeo, oscMat);
      osc.position.set(timerBaseX, y, 0.35);
      osc.userData = {
        type: 'task_box',
        task,
        label: task.label,
        interactive: true,
      };
      this.taskGroup.add(osc);
      this.chipMeshes.push(osc);

      // Square wave decoration
      const wavePts = [];
      for (let w = 0; w < 5; w++) {
        const wx = timerBaseX - 1 + w * 0.4;
        wavePts.push(new THREE.Vector3(wx, y + (w % 2 === 0 ? 0.15 : -0.15), 0.4));
        wavePts.push(new THREE.Vector3(wx + 0.2, y + (w % 2 === 0 ? 0.15 : -0.15), 0.4));
        wavePts.push(new THREE.Vector3(wx + 0.2, y + ((w + 1) % 2 === 0 ? 0.15 : -0.15), 0.4));
      }
      const waveGeo = new THREE.BufferGeometry().setFromPoints(wavePts);
      const waveMat = new THREE.LineBasicMaterial({ color: 0xd29922, transparent: true, opacity: 0.6 });
      this.taskGroup.add(new THREE.Line(waveGeo, waveMat));

      const nameLbl = this._makeLabel(task.label, 0.24, '#d29922');
      nameLbl.position.set(timerBaseX, y + 0.5, 0.4);
      this.taskGroup.add(nameLbl);

      const periodLbl = this._makeLabel(task.period, 0.2, '#8b949e');
      periodLbl.position.set(timerBaseX, y - 0.5, 0.4);
      this.taskGroup.add(periodLbl);
    });

    const timerLabel = this._makeLabel('Background Timers', 0.28, '#d29922');
    timerLabel.position.set(timerBaseX, timerBaseY + 1.2, 0.4);
    this.taskGroup.add(timerLabel);

    // ── Cancellation Token power rail (top edge) ─────────────────────
    const railMat = new THREE.MeshBasicMaterial({ color: 0xff4444, transparent: true, opacity: 0.5 });
    const railGeo = new THREE.PlaneGeometry(28, 0.15);
    const rail = new THREE.Mesh(railGeo, railMat);
    rail.position.set(0, 7.5, 0.35);
    this.taskGroup.add(rail);

    const railLabel = this._makeLabel('CancellationToken (root) — ShutdownHandle::shutdown() cancels all', 0.22, '#ff7b72');
    railLabel.position.set(0, 7.9, 0.4);
    this.taskGroup.add(railLabel);

    // Drop-down lines from rail to each permanent task
    const dropColor = 0xff4444;
    const dropMat = new THREE.LineBasicMaterial({ color: dropColor, transparent: true, opacity: 0.25 });
    // Pipeline runner
    this._drawTaskLane(0, 7.5, 0, 0.6, dropColor, 0.15);
    // Timer tasks
    timerTasks.forEach((_, i) => {
      this._drawTaskLane(timerBaseX, 7.5, timerBaseX, timerBaseY - i * 1.5 + 0.35, dropColor, 0.15);
    });

    // ── HTML overlay panel ───────────────────────────────────────────
    this.taskOverlay = document.createElement('div');
    this.taskOverlay.className = 'arch-task-overlay';
    this.taskOverlay.style.display = 'none';
    this.taskOverlay.innerHTML = `
      <div class="task-legend">
        <span class="task-legend-item"><span class="task-dot" style="background:#58a6ff"></span> IO-bound (face recv/send)</span>
        <span class="task-legend-item"><span class="task-dot" style="background:#d2a8ff"></span> CPU-bound (pipeline stages)</span>
        <span class="task-legend-item"><span class="task-dot" style="background:#d29922"></span> Timer (periodic background)</span>
        <span class="task-legend-item"><span class="task-dot" style="background:#ff7b72"></span> CancellationToken rail</span>
        <span class="task-legend-item"><span class="task-dot" style="background:#c87533"></span> mpsc channel bus</span>
      </div>
    `;
    this.container.appendChild(this.taskOverlay);
  }

  _drawTaskLane(x1, y1, x2, y2, color, opacity) {
    const pts = [new THREE.Vector3(x1, y1, 0.35), new THREE.Vector3(x2, y2, 0.35)];
    const geo = new THREE.BufferGeometry().setFromPoints(pts);
    const mat = new THREE.LineBasicMaterial({ color, transparent: true, opacity });
    this.taskGroup.add(new THREE.Line(geo, mat));
  }

  // ── Helpers ──────────────────────────────────────────────────────────────

  _makeLabel(text, size, color) {
    const div = document.createElement('div');
    div.textContent = text;
    div.style.cssText = `
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: ${size * 14}px;
      color: ${color};
      pointer-events: none;
      white-space: nowrap;
      text-shadow: 0 0 4px rgba(0,0,0,0.8);
    `;
    return new CSS2DObject(div);
  }

  _resize() {
    if (!this.renderer) return;
    const w = this.container.clientWidth;
    const h = this.container.clientHeight || 600;
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
    this.labelRenderer.setSize(w, h);
  }

  _startAnimation() {
    this.animating = true;
    const animate = () => {
      if (!this.animating) return;
      this.animationId = requestAnimationFrame(animate);
      this.controls.update();
      this.renderer.render(this.scene, this.camera);
      this.labelRenderer.render(this.scene, this.camera);
    };
    animate();
  }

  _stopAnimation() {
    this.animating = false;
    if (this.animationId) cancelAnimationFrame(this.animationId);
  }
}

// ── Module-level helpers ──────────────────────────────────────────────────

/** Parse a timescale string like "681 ns" or "1.40 µs" into nanoseconds. */
function parseTimescale(str) {
  if (!str || typeof str !== 'string') return 0;
  const match = str.match(/([\d.]+)\s*(ns|µs|us|ms|s)/);
  if (!match) return 0;
  const val = parseFloat(match[1]);
  switch (match[2]) {
    case 'ns': return val;
    case 'µs': case 'us': return val * 1000;
    case 'ms': return val * 1_000_000;
    case 's':  return val * 1_000_000_000;
    default:   return 0;
  }
}

/** Format nanoseconds for display: e.g. 1730 → "1.73 µs" */
function formatNs(ns) {
  if (ns < 1000) return `${Math.round(ns)} ns`;
  if (ns < 1_000_000) return `${(ns / 1000).toFixed(2)} µs`;
  if (ns < 1_000_000_000) return `${(ns / 1_000_000).toFixed(2)} ms`;
  return `${(ns / 1_000_000_000).toFixed(2)} s`;
}
