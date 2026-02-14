pub const TEST_PAGE_HTML: &str = r#"<!DOCTYPE html>
<html>
<head>
<meta charset="utf-8">
<title>streambridge test</title>
<style>
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { font-family: system-ui, sans-serif; background: #1a1a2e; color: #e0e0e0; padding: 20px; }
  h1 { margin-bottom: 16px; font-size: 1.4em; color: #fff; }
  .toolbar { margin-bottom: 16px; display: flex; gap: 8px; }
  button {
    padding: 6px 14px; border: 1px solid #444; border-radius: 4px;
    background: #2a2a4a; color: #e0e0e0; cursor: pointer; font-size: 0.9em;
  }
  button:hover { background: #3a3a5a; }
  #source-list { margin-bottom: 20px; }
  .source-btn {
    display: inline-block; margin: 4px; padding: 8px 16px;
    border: 1px solid #555; border-radius: 4px; background: #2a2a4a;
    color: #e0e0e0; cursor: pointer; font-size: 0.9em;
  }
  .source-btn:hover { background: #3a3a5a; }
  .source-btn.active { border-color: #6c6cff; background: #3a3a6a; }
  #previews { display: flex; flex-wrap: wrap; gap: 16px; }
  .preview {
    background: #222244; border-radius: 6px; overflow: hidden;
    border: 1px solid #333;
  }
  .preview-header {
    padding: 8px 12px; font-size: 0.85em; display: flex;
    justify-content: space-between; align-items: center; background: #1a1a3e;
  }
  .preview-close { cursor: pointer; color: #aaa; font-size: 1.1em; }
  .preview-close:hover { color: #fff; }
  .preview img { display: block; max-width: 640px; height: auto; }
  .info { margin-top: 40px; max-width: 800px; }
  .info h2 { font-size: 1.15em; color: #fff; margin: 24px 0 8px; border-bottom: 1px solid #333; padding-bottom: 4px; }
  .info h2:first-child { margin-top: 0; }
  .info p, .info ul { margin: 6px 0; line-height: 1.5; font-size: 0.9em; color: #ccc; }
  .info ul { padding-left: 20px; }
  .info code { background: #2a2a4a; padding: 1px 5px; border-radius: 3px; font-size: 0.85em; }
  .info pre {
    background: #16162e; border: 1px solid #333; border-radius: 4px;
    padding: 12px; overflow-x: auto; font-size: 0.82em; line-height: 1.45; margin: 8px 0;
  }
  .info pre code { background: none; padding: 0; }
  .ndi-attr { margin-top: 32px; padding-top: 12px; border-top: 1px solid #333; font-size: 0.78em; color: #888; }
</style>
</head>
<body>
<h1>streambridge test</h1>
<div class="toolbar">
  <button onclick="refreshSources()">Refresh Sources</button>
  <button onclick="clearAll()">Clear All</button>
</div>
<div id="source-list"></div>
<div id="previews"></div>
<script>
const wsProto = location.protocol === 'https:' ? 'wss:' : 'ws:';
const baseUrl = location.origin;
const wsBase = wsProto + '//' + location.host;
let connections = {};

async function refreshSources() {
  try {
    const res = await fetch(baseUrl + '/sources');
    const sources = await res.json();
    const el = document.getElementById('source-list');
    el.innerHTML = '';
    sources.forEach(name => {
      const btn = document.createElement('button');
      btn.className = 'source-btn' + (connections[name] ? ' active' : '');
      btn.textContent = name;
      btn.onclick = () => togglePreview(name);
      el.appendChild(btn);
    });
  } catch (e) {
    console.error('Failed to fetch sources:', e);
  }
}

function togglePreview(name) {
  if (connections[name]) {
    closePreview(name);
  } else {
    openPreview(name);
  }
}

function openPreview(name) {
  const previews = document.getElementById('previews');
  const div = document.createElement('div');
  div.className = 'preview';
  div.id = 'preview-' + CSS.escape(name);

  const header = document.createElement('div');
  header.className = 'preview-header';
  header.innerHTML = '<span>' + name + '</span><span class="preview-close" onclick="closePreview(\'' + name.replace(/'/g, "\\'") + '\')">&times;</span>';

  const img = document.createElement('img');
  div.appendChild(header);
  div.appendChild(img);
  previews.appendChild(div);

  const ws = new WebSocket(wsBase + '/ws?source=' + encodeURIComponent(name));
  ws.binaryType = 'arraybuffer';
  ws.onmessage = (e) => {
    const blob = new Blob([e.data], { type: 'image/jpeg' });
    const url = URL.createObjectURL(blob);
    const oldUrl = img.src;
    img.src = url;
    if (oldUrl && oldUrl.startsWith('blob:')) URL.revokeObjectURL(oldUrl);
  };
  ws.onclose = (e) => {
    console.log('WS closed for', name, e.code, e.reason);
    delete connections[name];
    updateButtons();
  };
  ws.onerror = () => { ws.close(); };

  connections[name] = { ws, div };
  updateButtons();
}

function closePreview(name) {
  const conn = connections[name];
  if (conn) {
    conn.ws.close();
    conn.div.remove();
    delete connections[name];
    updateButtons();
  }
}

function clearAll() {
  Object.keys(connections).forEach(closePreview);
}

function updateButtons() {
  document.querySelectorAll('.source-btn').forEach(btn => {
    btn.className = 'source-btn' + (connections[btn.textContent] ? ' active' : '');
  });
}

refreshSources();
</script>

<div class="info">
  <h2>API Reference</h2>
  <ul>
    <li><code>GET /sources</code> &mdash; returns a JSON array of NDI source names currently visible on the network.</li>
    <li><code>WebSocket /ws?source=&lt;name&gt;</code> &mdash; streams binary JPEG frames for the given source. Each WebSocket message is one complete JPEG image.</li>
  </ul>

  <h2>Browser Usage Example</h2>
  <p>Connect to a source and display frames in an <code>&lt;img&gt;</code> tag:</p>
  <pre><code>const img = document.getElementById('my-img');
const ws = new WebSocket('ws://localhost:9550/ws?source=MY_SOURCE');
ws.binaryType = 'arraybuffer';
ws.onmessage = (e) => {
  const blob = new Blob([e.data], { type: 'image/jpeg' });
  const url = URL.createObjectURL(blob);
  const old = img.src;
  img.src = url;
  if (old.startsWith('blob:')) URL.revokeObjectURL(old);
};</code></pre>

  <div class="ndi-attr">
    Powered by NDI&reg; &mdash; NDI&reg; is a registered trademark of Vizrt NDI AB.
  </div>
</div>
</body>
</html>
"#;
