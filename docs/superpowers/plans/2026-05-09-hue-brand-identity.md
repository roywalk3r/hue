# HUE ENGINE ❄
### System Designation: The Architecture of Aggregation

# Hue Stream API - "The Grid" Brand Identity Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish a cohesive, high-tech "Cyberpunk / The Grid" brand identity across the README, API documentation, and system interfaces.

**Architecture:** This is a visual and content-focused refactor. We are updating CSS variables, HTML templates, and markdown content to shift from a generic streaming look to a professional "engineered engine" aesthetic (The Grid).

**Tech Stack:** Markdown, CSS (Custom Properties), Node.js (Express), Scalar (API Reference).

---

### Task 1: README.md Refactor

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Replace header with "The Grid" ASCII logo and Technical Manual tone**

Update the top section of `README.md` to use a technical manual structure.

```markdown
# HUE STREAM API ⬢
### System Designation: The Architecture of Aggregation
**Version:** 1.8.4 | **Status:** Nominal | **Engine:** Node.js v18+

> [!IMPORTANT]
> **Manual-First Backend Engine:** Hue is a high-performance streaming aggregator. It is engineered for modularity, resilience, and schema-strict data delivery. This is not a media player; it is the infrastructure that powers them.

---

## ⬢ System Specifications

### 01. Modular Provider Registry
The Hue core operates as a central hub for independent provider nodes.
*   **Nova (SFlix):** High-speed spectral aggregation.
*   **Zenith (KissKH):** Focused regional drama telemetry.
*   **Stadium (Livesport):** Real-time sports data streams.
*   **Nova/Cinema/Luna:** Primary high-fidelity content sources.

### 02. Secure Blueprinting (Proxy)
All outbound telemetry is encapsulated via an AES-256 HMAC signed proxy system.
*   **Telemetry Locking:** Origin-restricted bandwidth management.
*   **IPv4 Failover:** Automatic route switching for legacy CDNs.

### 03. Discovery Schematic
A multi-layered catalog engine mapping global content (HiTV/MovieBox) to playable provider nodes.
```

- [ ] **Step 2: Commit README changes**

```bash
git add README.md
git commit -m "docs: refactor README to 'The Grid' brand identity"
```

---

### Task 2: API Documentation (Scalar) Customization

**Files:**
- Modify: `apiServer.js`

- [ ] **Step 1: Update Scalar theme and colors in `apiServer.js`**

Inject custom CSS into Scalar to match the Engine Green and Charcoal palette.

```javascript
// Find app.use('/api/docs/scalar' ...
// Update to:
app.use('/api/docs/scalar', apiReference({
    spec: { url: '/api/docs/openapi.json' },
    theme: 'none', // Use custom theme
    hideDownloadButton: true,
    pageTitle: 'HUE ENGINE ⬢ API REFERENCE',
    customCss: `
      :root {
        --scalar-color-1: #E0E0E0;
        --scalar-color-2: #aaa;
        --scalar-color-3: #777;
        --scalar-color-accent: #1A5E3A;
        --scalar-background-1: #0A0A0A;
        --scalar-background-2: #121212;
        --scalar-background-3: #1a1a1a;
        --scalar-border-color: #2A2A2A;
        --scalar-font: 'JetBrains Mono', 'Fira Code', monospace;
      }
      .scalar-card { border: 1px solid var(--scalar-border-color) !important; }
      .sidebar { background: var(--scalar-background-2) !important; border-right: 1px solid var(--scalar-border-color) !important; }
    `
}));
```

- [ ] **Step 2: Commit Documentation changes**

```bash
git add apiServer.js
git commit -m "style: apply 'The Grid' theme to Scalar API docs"
```

---

### Task 3: OpenAPI Content Refactor

**Files:**
- Modify: `utils/openapi-spec.js`

- [ ] **Step 1: Update API descriptions to match "Manual-first" voice**

```javascript
// Modify the buildOpenApiSpec return object:
info: {
  title: 'HUE ENGINE ⬢',
  version: pkg.version,
  description: 'Technical Manual for the Hue Stream Aggregation Engine. ' +
    'Engineered for high-performance stream telemetry across decentralized providers. ' +
    'Authorized access only. Secure HMAC signing enforced on all proxy egress.',
  contact: { name: 'System Administrator' },
},
```

- [ ] **Step 2: Update tags for a technical feel**

```javascript
tags: [
  { name: 'Telemetry', description: 'Real-time stream aggregation and resolution.' },
  { name: 'Registry', description: 'Provider node management and status.' },
  { name: 'Gateway', description: 'Smart mapping for regional discovery systems.' },
  // ... update others to be less "friendly" and more "technical"
],
```

- [ ] **Step 3: Commit OpenAPI updates**

```bash
git add utils/openapi-spec.js
git commit -m "docs: update OpenAPI spec with 'The Grid' brand voice"
```

---

### Task 4: System Interfaces (CSS & HTML) Refactor

**Files:**
- Modify: `public/style.css`
- Modify: `public/index.html`

- [ ] **Step 1: Update CSS variables in `public/style.css`**

```css
:root {
  --bg: #050505;
  --bg-alt: #0a0a0a;
  --bg-accent: #121212;
  --border: #1A5E3A; /* Engine Green */
  --border-strong: #1A5E3A;
  --text: #E0E0E0;
  --text-dim: #888;
  --primary: #1A5E3A;
  --primary-accent: #2ecc71;
  --primary-fg: #ffffff;
  --danger: #e74c3c;
  --warn: #FFBF00; /* Warning Amber */
  --mono: 'JetBrains Mono', monospace;
}
/* Force dark mode variables even if no theme data set */
body { background: var(--bg); color: var(--text); border-top: 2px solid var(--primary); }
```

- [ ] **Step 2: Update `public/index.html` to reflect "Dashboard" feel**

Change navbar brand and search button text.

```html
<!-- Navbar -->
<a class="navbar-brand fw-bold text-primary" href="#">HUE ENGINE ⬢</a>

<!-- Search Button -->
<button class="btn btn-primary px-4" id="searchBtn">INITIALIZE SCAN</button>

<!-- Update Placeholder -->
<input type="text" ... placeholder="SCAN TELEMETRY OR ENTER NODE ID...">
```

- [ ] **Step 3: Commit Interface changes**

```bash
git add public/style.css public/index.html
git commit -m "style: apply 'The Grid' aesthetic to system interfaces"
```

---

### Task 5: Final Validation

- [ ] **Step 1: Verify README rendering**
- [ ] **Step 2: Check `/api/docs` and confirm Scalar theme loads correctly**
- [ ] **Step 3: Check homepage (`/`) and confirm colors/fonts match "The Grid"**
