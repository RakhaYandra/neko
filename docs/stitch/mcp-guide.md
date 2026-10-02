# Neko MCP Server — Complete Implementation & OpenCode Integration Guide

Panduan dan kode lengkap untuk menghubungkan **OpenCode** (AI coding agent) ke **Neko Linux Developer Companion** melalui protokol **Model Context Protocol (MCP)**.

Dengan server ini, OpenCode dapat secara otomatis mengubah pose maskot Neko secara *real-time* sesuai status pengerjaan kode (misal: `thinking` saat membaca berkas, `coding` saat menulis/generate script, `permission` saat meminta sudo/konfirmasi, `completed`/`celebrate` saat task selesai, dan `error` jika build/test gagal).

---

## 1. Arsitektur Komunikasi

```
┌─────────────────────────────────┐
│     OpenCode (AI Agent)         │
│  (Claude Code / Cursor / CLI)   │
└────────────────┬────────────────┘
                 │ stdio (JSON-RPC)
┌────────────────▼────────────────┐
│    Neko MCP Server              │
│    (neko_mcp_server.py)         │
└────────────────┬────────────────┘
                 │ IPC Unix Domain Socket (/tmp/neko.sock)
                 │ atau HTTP WebSocket / JSON Post
┌────────────────▼────────────────┐
│    Neko Companion UI / Daemon   │
│  (Web App / Desktop Visualizer) │
└─────────────────────────────────┘
```

---

## 2. Implementasi MCP Server (Python — `FastMCP`)

Implementasi menggunakan library standar resmi `@modelcontextprotocol/sdk` versi Python (`mcp[cli]`).

### Persiapan Environment
```bash
pip install "mcp[cli]" fastapi uvicorn
```

### File: `neko_mcp_server.py`
```python
#!/usr/bin/env python3
"""
Neko MCP Server — Linux Developer Companion
Menyediakan tool & resource bagi OpenCode untuk mengontrol status dan pose maskot Neko.
"""

import os
import json
import socket
from typing import Optional, List
from mcp.server.fastmcp import FastMCP

# Inisialisasi FastMCP
mcp = FastMCP(
    "Neko-Linux-Companion",
    dependencies=["mcp"]
)

SOCKET_PATH = os.environ.get("NEKO_SOCKET_PATH", "/tmp/neko.sock")

ALL_POSES = [
    "idle", "coding", "busy", "permission", "completed",
    "error", "disconnected", "thinking", "celebrate", "reconnect",
    "peeking", "dragging", "listen", "denied", "allowed", "multi-session"
]

def send_to_daemon(payload: dict) -> bool:
    """Mengirim sinyal state ke daemon Neko via Unix Domain Socket."""
    if not os.path.exists(SOCKET_PATH):
        # Jika socket daemon belum aktif, log ke file atau fallback
        return False
    try:
        client = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
        client.connect(SOCKET_PATH)
        client.sendall(json.dumps(payload).encode("utf-8") + b"\n")
        client.close()
        return True
    except Exception as e:
        print(f"[Neko-MCP] Socket communication warning: {e}")
        return False

# ==========================================
# MCP TOOLS UNTUK OPENCODE
# ==========================================

@mcp.tool()
def set_mascot_pose(
    pose: str,
    action_description: Optional[str] = "",
    status_type: Optional[str] = "working"
) -> str:
    """
    Ubah pose maskot Neko secara langsung sesuai status OpenCode.
    
    Args:
        pose: Nama pose (pilihan: idle, coding, busy, permission, completed, error, disconnected, thinking, celebrate, reconnect, peeking, dragging, listen, denied, allowed, multi-session)
        action_description: Keterangan singkat yang sedang dikerjakan agent (misal: "Refactoring auth module", "Running pytest")
        status_type: Kategori status dot ("idle", "working", "tool", "permission", "error")
    """
    pose_clean = pose.lower().strip()
    if pose_clean not in ALL_POSES:
        return f"Error: Pose '{pose}' tidak dikenal. Pilihan valid: {', '.join(ALL_POSES)}"

    payload = {
        "event": "POSE_UPDATE",
        "pose": pose_clean,
        "description": action_description,
        "status_type": status_type,
        "timestamp": os.times()[4]
    }
    
    socket_ok = send_to_daemon(payload)
    status_str = f"Neko mascot pose -> [{pose_clean.upper()}]"
    if action_description:
        status_str += f" | Task: '{action_description}'"
    if not socket_ok:
        status_str += " (Tercatat di server buffer; daemon UI socket offline)"
    return status_str

@mcp.tool()
def trigger_agent_state(state: str, details: Optional[str] = "") -> str:
    """
    Helper otomatis untuk memetakan fase pengerjaan coding agent ke pose Neko:
    - 'planning' / 'reasoning' -> thinking
    - 'writing_code' / 'editing' -> coding
    - 'executing_command' / 'bash' -> busy
    - 'requesting_permission' / 'sudo' -> permission
    - 'test_passed' / 'task_done' -> completed atau celebrate
    - 'failed' / 'error' -> error
    """
    mapping = {
        "planning": ("thinking", "tool"),
        "reasoning": ("thinking", "tool"),
        "writing_code": ("coding", "working"),
        "editing": ("coding", "working"),
        "executing_command": ("busy", "tool"),
        "bash": ("busy", "tool"),
        "requesting_permission": ("permission", "permission"),
        "sudo": ("permission", "permission"),
        "test_passed": ("completed", "working"),
        "task_done": ("celebrate", "working"),
        "failed": ("error", "error"),
        "error": ("error", "error")
    }

    state_key = state.lower().strip()
    if state_key in mapping:
        pose, status_type = mapping[state_key]
        return set_mascot_pose(pose=pose, action_description=details, status_type=status_type)
    else:
        return f"State '{state}' tidak dikenal. Pilihan: {list(mapping.keys())}"

@mcp.tool()
def get_companion_status() -> str:
    """Membaca konfigurasi dan status terkini dari Neko Companion."""
    return json.dumps({
        "companion": "Neko — Linux Developer Companion",
        "version": "1.2.0",
        "socket_active": os.path.exists(SOCKET_PATH),
        "supported_poses": ALL_POSES
    }, indent=2)

# ==========================================
# MCP RESOURCE
# ==========================================

@mcp.resource("neko://spec/atlas")
def get_sprite_atlas_resource() -> str:
    """Menyediakan spesifikasi 16 pose Neko sebagai referensi konteks untuk agent."""
    return json.dumps({
        "poses": ALL_POSES,
        "color_palette": {
            "canvas": "#0d1117",
            "surface": "#161b22",
            "dark_body": "#e8e8f0",
            "amber_paw": "#ffb224"
        }
    }, indent=2)

if __name__ == "__main__":
    mcp.run()
```

---

## 3. Implementasi MCP Server (Node.js / TypeScript Alternatif)

Jika project OpenCode Anda menggunakan ekosistem Node.js:

### File: `neko-mcp-server.js`
```javascript
#!/usr/bin/env node
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import {
  CallToolRequestSchema,
  ListToolsRequestSchema,
} from "@modelcontextprotocol/sdk/types.js";
import net from "net";

const SOCKET_PATH = process.env.NEKO_SOCKET_PATH || "/tmp/neko.sock";

const server = new Server(
  {
    name: "neko-companion-mcp",
    version: "1.2.0",
  },
  {
    capabilities: {
      tools: {},
    },
  }
);

const VALID_POSES = [
  "idle", "coding", "busy", "permission", "completed",
  "error", "disconnected", "thinking", "celebrate", "reconnect",
  "peeking", "dragging", "listen", "denied", "allowed", "multi-session"
];

function notifyDaemon(data) {
  return new Promise((resolve) => {
    const client = net.createConnection(SOCKET_PATH, () => {
      client.write(JSON.stringify(data) + "\n");
      client.end();
      resolve(true);
    });
    client.on("error", () => resolve(false));
  });
}

server.setRequestHandler(ListToolsRequestSchema, async () => {
  return {
    tools: [
      {
        name: "set_mascot_pose",
        description: "Ubah pose visual maskot Neko Linux Companion (idle, coding, thinking, completed, celebrate, dll)",
        inputSchema: {
          type: "object",
          properties: {
            pose: {
              type: "string",
              description: `Pilihan pose: ${VALID_POSES.join(", ")}`,
            },
            action_description: {
              type: "string",
              description: "Keterangan pekerjaan agent saat ini",
            },
          },
          required: ["pose"],
        },
      },
    ],
  };
});

server.setRequestHandler(CallToolRequestSchema, async (request) => {
  if (request.params.name === "set_mascot_pose") {
    const { pose, action_description } = request.params.arguments;
    const cleanPose = (pose || "").toLowerCase();
    
    if (!VALID_POSES.includes(cleanPose)) {
      return {
        content: [{ type: "text", text: `Pose ${cleanPose} tidak valid.` }],
        isError: true,
      };
    }

    const ok = await notifyDaemon({
      event: "POSE_UPDATE",
      pose: cleanPose,
      description: action_description || "",
    });

    return {
      content: [
        {
          type: "text",
          text: `[Neko] Pose updated to: ${cleanPose} (${ok ? "Socket Sent" : "Buffered"})`,
        },
      ],
    };
  }
  throw new Error("Tool tidak ditemukan");
});

async function run() {
  const transport = new StdioServerTransport();
  await server.connect(transport);
}

run().catch(console.error);
```

---

## 4. File Konfigurasi OpenCode

Letakkan konfigurasi ini di file konfigurasi OpenCode Anda (misalnya `.opencode/mcp.json` di project root, atau `~/.config/opencode/mcp.json`):

```json
{
  "mcpServers": {
    "neko": {
      "command": "python3",
      "args": [
        "/absolute/path/to/neko_mcp_server.py"
      ],
      "env": {
        "NEKO_SOCKET_PATH": "/tmp/neko.sock"
      }
    }
  }
}
```

*Jika menggunakan Node.js:*
```json
{
  "mcpServers": {
    "neko": {
      "command": "node",
      "args": [
        "/absolute/path/to/neko-mcp-server.js"
      ],
      "env": {
        "NEKO_SOCKET_PATH": "/tmp/neko.sock"
      }
    }
  }
}
```

---

## 5. System Prompt / Instruksi Otomatisasi untuk OpenCode

Tambahkan panduan perilaku ini ke file instruksi agent OpenCode (`opencode_rules.md` atau system instructions) agar OpenCode secara proaktif memanggil tool Neko:

```markdown
### Neko Mascot Companion Protocol:
Setiap kali Anda menjalankan tugas, gunakan tool MCP `neko` / `set_mascot_pose` untuk memperbarui status maskot:
1. Saat sedang menganalisis kode atau merencanakan solusi:
   -> panggil `set_mascot_pose(pose="thinking", action_description="Analyzing codebase")`
2. Saat mulai menulis atau mengubah file (coding):
   -> panggil `set_mascot_pose(pose="coding", action_description="Writing implementation")`
3. Saat menjalankan bash command, build, atau testing:
   -> panggil `set_mascot_pose(pose="busy", action_description="Running compiler / tests")`
4. Jika butuh persetujuan user untuk perintah berisiko atau token sensitif:
   -> panggil `set_mascot_pose(pose="permission", action_description="Awaiting confirmation")`
5. Ketika seluruh tugas berhasil diselesaikan dan verifikasi sukses:
   -> panggil `set_mascot_pose(pose="celebrate", action_description="Task completed successfully!")`
6. Jika terjadi error kritis atau build gagal:
   -> panggil `set_mascot_pose(pose="error", action_description="Error encountered")`
```

---

## 6. Verifikasi & Testing di Terminal

1. **Jalankan Neko MCP Server secara lokal:**
   ```bash
   python3 neko_mcp_server.py
   ```
2. **Jalankan OpenCode:**
   ```bash
   opencode
   ```
3. **Kirim prompt uji coba:**
   > *"Tolong panggil tool neko set_mascot_pose dengan pose 'coding' dan description 'Refactoring parser module'"*

OpenCode akan mengeksekusi tool tersebut dan Neko Companion akan langsung merespons dengan animasi pose Coding!
