# Neko — Linux Developer Companion
## Implementation & Design Specification Guide for OpenCode / AI Coding Agent

Dokumen ini dirancang agar coding agent (OpenCode, Claude Code, Cursor, Copilot) dapat mereproduksi antarmuka dan sistem sprite **Neko** secara 100% presisi dan identik (*pixel-perfect*).

---

### 1. Project Overview & Aesthetic
- **Nama Aplikasi**: Neko — Linux Developer Companion
- **Deskripsi**: 16×16 pixel-art mascot system & headless daemon status visualizer untuk Linux developers.
- **Visual Direction**: GitHub dark theme / retro Linux terminal console, dark slate canvas, high-contrast monospace typography, crisp non-anti-aliased pixel graphics.
- **Font**: Monospace priority (`ui-monospace, SFMono-Regular, "JetBrains Mono", "Fira Code", monospace`).

---

### 2. Design Tokens & Color Palette

Gunakan token warna berikut pada Tailwind config atau CSS Variables:

```json
{
  "theme": {
    "colors": {
      "canvas": "#0d1117",
      "surface": "#161b22",
      "surface-hover": "#21262d",
      "border-default": "#30363d",
      "border-active": "#ffb224",
      "text-primary": "#f0f6fc",
      "text-secondary": "#8b949e",
      "text-muted": "#6e7681",
      "status": {
        "idle": "#8b949e",
        "working": "#3fb950",
        "tool": "#58a6ff",
        "permission": "#d29922",
        "error": "#f85149"
      },
      "mascot": {
        "dark-body": "#e8e8f0",
        "light-body": "#23232e",
        "ink-outline": "#16161e",
        "amber-accent": "#ffb224",
        "shadow": "#c0c0d0"
      }
    }
  }
}
```

---

### 3. Critical CSS Rule: Pixel-Perfect Rendering

Wajib ditambahkan pada global CSS agar gambar 16×16 tidak blur saat diperbesar:

```css
/* Wajib untuk rendering retro pixel-art tanpa blur/smoothing */
.pixel-art,
img[src*="pixel"],
svg.pixel-art,
canvas.pixel-art {
  image-rendering: pixelated;
  image-rendering: crisp-edges;
  image-rendering: -moz-crisp-edges;
  shape-rendering: crispEdges;
}
```

---

### 4. Sprite Atlas: 16 Core Poses & State Mapping

Setiap pose memiliki status indicator dot dan aturan animasi:

| # | Pose ID | Status Dot | Visual Details & Particles | Frames | Behavior |
|---|---------|------------|----------------------------|--------|----------|
| 1 | `idle` | Gray (`#8b949e`) | Mata horizontal `– –`, partikel amber `Zzz` melayang | 2 Frames | Looping lembut |
| 2 | `coding` | Green (`#3fb950`) | Paws bergantian atas/bawah di atas mini terminal baseline | 2 Frames | Looping konstan saat build |
| 3 | `busy` | Blue (`#58a6ff`) | Mata squint `> <` dengan ikon kunci inggris di samping | 2 Frames | Looping saat script aktif |
| 4 | `permission` | Amber (`#d29922`) | Telinga tegak waspada dengan tanda seru `!` di atas kepala | 1 Frame + Bounce | Looping bounce |
| 5 | `completed` | Green (`#3fb950`) | Pose melompat gembira dengan not balok `♪` | 2 Frames | Looping / success state |
| 6 | `error` | Red (`#f85149`) | Telinga terkulai, mata merah `X X`, mulut sedih | 1 Frame | Statis saat exit non-zero |
| 7 | `disconnected` | Gray (`#8b949e`) | Warna monokrom / grayscale dengan ikon kabel terputus | 1 Frame | Statis saat daemon lost |
| 8 | `thinking` | Blue (`#58a6ff`) | Tatapan ke atas, satu paw di dagu, partikel `…` | 2 Frames | Looping saat LLM reasoning |
| 9 | `celebrate` | Green (`#3fb950`) | Kedua paws ke atas, partikel konfeti warna-warni | 3 Frames | **Play once** (tidak loop) |
| 10 | `reconnect` | Blue (`#58a6ff`) | Mata berkedip setengah, kabel menyambung ke samping | 2 Frames | Looping saat retry socket |
| 11 | `peeking` | Gray (`#8b949e`) | Tubuh setengah tersembunyi, mata mengintip dari bezel bawah | 1 Frame | Statis background daemon |
| 12 | `dragging` | Amber (`#d29922`) | Pose miring menahan cengkeraman, garis akselerasi gerak | 2 Frames | Aktif saat window drag |
| 13 | `listen` | Blue (`#58a6ff`) | Telinga kiri miring dengan sinyal gelombang audio | 2 Frames | Looping stdin listener |
| 14 | `denied` | Red (`#f85149`) | Paws menyilang membentuk huruf `X` di dada | 2 Frames (Shake) | Durasi max 600ms lalu reset |
| 15 | `allowed` | Green (`#3fb950`) | Jempol ke atas (*thumbs-up*) dengan tanda centang `✓` | 1 Frame | Durasi max 600ms lalu reset |
| 16 | `multi-session` | Blue (`#58a6ff`) | Kucing utama dengan bayangan siluet 2 sub-daemon di belakang | Static | Menunjukkan multi-worker |

---

### 5. SVG Pixel Mascot Definition (Master Template)

Gunakan template SVG inline berikut untuk merender Neko secara dinamis (contoh pose Idle & Coding):

```html
<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16" width="64" height="64" class="pixel-art" shape-rendering="crispEdges">
  <!-- Ears -->
  <rect x="3" y="3" width="3" height="3" fill="#16161e"/>
  <rect x="10" y="3" width="3" height="3" fill="#16161e"/>
  <rect x="4" y="4" width="1" height="1" fill="#ffb224"/>
  <rect x="11" y="4" width="1" height="1" fill="#ffb224"/>
  
  <!-- Head & Body -->
  <rect x="2" y="5" width="12" height="9" fill="#16161e"/>
  <rect x="3" y="6" width="10" height="7" fill="#e8e8f0"/>
  
  <!-- Eyes (Active / Focused) -->
  <rect x="5" y="8" width="1" height="2" fill="#16161e"/>
  <rect x="10" y="8" width="1" height="2" fill="#16161e"/>
  
  <!-- Nose/Mouth -->
  <rect x="7" y="9" width="2" height="1" fill="#ffb224"/>
  
  <!-- Terminal Keyboard baseline -->
  <rect x="4" y="13" width="8" height="1" fill="#58a6ff"/>
  <!-- Alternating Paws -->
  <rect x="4" y="11" width="2" height="2" fill="#ffb224"/>
  <rect x="10" y="12" width="2" height="1" fill="#ffb224"/>
</svg>
```

---

### 6. PWA & Web Deployment Manifest

Letakkan file `manifest.json` di root public direktori:

```json
{
  "name": "Neko — Linux Developer Companion",
  "short_name": "Neko",
  "description": "16×16 pixel-art mascot system & headless daemon status visualizer for Linux developers.",
  "start_url": "/",
  "display": "standalone",
  "orientation": "portrait-primary",
  "background_color": "#0d1117",
  "theme_color": "#161b22",
  "icons": [
    {
      "src": "/icons/icon-192x192.png",
      "sizes": "192x192",
      "type": "image/png",
      "purpose": "any maskable"
    },
    {
      "src": "/icons/icon-512x512.png",
      "sizes": "512x512",
      "type": "image/png",
      "purpose": "any maskable"
    },
    {
      "src": "/icons/apple-touch-icon.png",
      "sizes": "180x180",
      "type": "image/png",
      "purpose": "any"
    }
  ]
}
```

---

### 7. Full UI HTML Template (Component Reference)

Gunakan struktur layout berikut untuk halaman utama:

```html
<!DOCTYPE html>
<html lang="en" class="dark">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>Neko Linux Mascot & Sprite Suite</title>
  <script src="https://cdn.tailwindcss.com"></script>
  <style>
    .pixel-art {
      image-rendering: pixelated;
      image-rendering: crisp-edges;
      shape-rendering: crispEdges;
    }
  </style>
</head>
<body class="bg-[#0d1117] text-[#f0f6fc] font-mono p-4 min-h-screen">

  <!-- Top Status Bar -->
  <header class="flex justify-between items-center mb-6 text-xs">
    <div class="inline-flex items-center gap-2 px-3 py-1 rounded-full border border-[#238636] bg-[#238636]/10 text-[#3fb950]">
      <span class="w-2 h-2 rounded-full bg-[#3fb950] animate-pulse"></span>
      <span>neko-daemon active</span>
    </div>
    <div class="flex items-center gap-1 bg-[#161b22] border border-[#30363d] p-1 rounded-lg">
      <button class="px-2 py-0.5 rounded bg-[#21262d] text-xs text-[#ffb224]">● Dark</button>
      <button class="px-2 py-0.5 rounded text-xs text-[#8b949e]">● Light</button>
    </div>
  </header>

  <!-- Title & Meta -->
  <section class="mb-6">
    <div class="flex items-baseline gap-2">
      <h1 class="text-xl font-bold tracking-wider">NEKO</h1>
      <span class="text-xs bg-[#21262d] text-[#ffb224] px-1.5 py-0.5 rounded font-mono border border-[#30363d]">v1.2.0</span>
      <span class="text-xs text-[#8b949e] ml-auto">Linux Dev Companion</span>
    </div>
    <p class="text-xs text-[#8b949e] mt-1">16×16 pixel mascot system & headless daemon status visualizer.</p>
  </section>

  <!-- Interactive Stage Card -->
  <section class="bg-[#161b22] border border-[#30363d] rounded-xl p-6 mb-6 text-center">
    <div class="flex justify-between text-xs mb-4">
      <span class="text-[#3fb950] flex items-center gap-1.5">
        <span class="w-2 h-2 rounded-full bg-[#3fb950]"></span> 2. CODING (PAWS)
      </span>
      <div class="flex gap-2 text-[#8b949e]">
        <span class="bg-[#0d1117] px-2 py-0.5 rounded border border-[#30363d]">2 FRAMES</span>
        <button class="bg-[#0d1117] text-[#ffb224] px-2 py-0.5 rounded border border-[#ffb224]/40 hover:bg-[#ffb224]/10">Trigger ▶</button>
      </div>
    </div>

    <!-- Active Stage Screen -->
    <div class="w-48 h-48 mx-auto bg-[#0d1117] border border-[#30363d] rounded-lg flex items-center justify-center relative shadow-inner">
      <div class="absolute inset-0 bg-[radial-gradient(#30363d_1px,transparent_1px)] [background-size:12px_12px] opacity-40"></div>
      <!-- Scaled Mascot (16x16 scaled to 64x64 or 96x96) -->
      <div class="relative z-10 scale-[5] pixel-art">
        <!-- Inline SVG Mascot here -->
      </div>
    </div>

    <div class="mt-4 flex justify-between items-center text-xs text-[#8b949e] pt-4 border-t border-[#21262d]">
      <div>Daemon Trigger: <span class="text-[#ffb224] font-semibold">Active code compilation / build</span></div>
      <div class="text-[10px] uppercase tracking-wider text-[#6e7681]">Dark Palette</div>
    </div>
  </section>

  <!-- CLI Integration Snippet -->
  <section class="bg-[#161b22] border border-[#30363d] rounded-lg p-3 text-xs">
    <div class="flex justify-between text-[#8b949e] mb-2 font-mono text-[11px]">
      <span class="text-[#3fb950] flex items-center gap-1">›_ CLI Daemon Integration</span>
      <span class="text-[#ffb224]">zsh/bash</span>
    </div>
    <div class="bg-[#0d1117] p-2.5 rounded font-mono text-[11px] text-[#8b949e] select-all space-y-1 border border-[#21262d]">
      <p><span class="text-[#ffb224]">$</span> curl -s https://neko.sh/install | bash</p>
      <p><span class="text-[#ffb224]">$</span> neko-daemon --attach --pose=<span class="text-[#3fb950]">coding</span></p>
    </div>
  </section>

</body>
</html>
```

---

### 8. Quick Prompt Instructions for OpenCode

Untuk menginstruksikan OpenCode Anda, salin dan kirimkan prompt ini:
> *"Tolong implementasikan komponen UI dan sprite sistem 'Neko Linux Mascot' sesuai dengan dokumen spesifikasi terlampir. Gunakan tema dark terminal (#0d1117 & #161b22), font monospace, styling Tailwind CSS, dan pastikan seluruh SVG / pixel art memiliki aturan CSS `image-rendering: pixelated` dan `shape-rendering: crispEdges` agar tidak buram."*
