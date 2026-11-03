#!/usr/bin/env python3
"""
Benchmark HTTP Test Server for Shadhin Browser.
Serves controlled test scenarios:
- /text.html: Simple static text content (~5KB)
- /heavy_dom.html: Heavy DOM with nested structures and SVG assets
- /timer.html: JavaScript timer with periodic DOM updates
- /animation.html: Continuous CSS keyframe animation
- /download.bin: 5MB dummy binary stream for download testing
"""

import http.server
import socketserver
import sys
import argparse

TEXT_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Benchmark: Text Content</title>
    <style>body { font-family: sans-serif; line-height: 1.6; padding: 2rem; max-width: 800px; margin: 0 auto; }</style>
</head>
<body>
    <h1>Shadhin Browser Benchmark: Plain Text</h1>
    <p>This page represents a light, text-dominated web page used to measure baseline rendering memory and CPU utilization.</p>
""" + "\n".join([f"<p>Paragraph {i}: The quick brown fox jumps over the lazy dog. Shadhin Browser provides memory-efficient, hardened browsing with strict workspace isolation and content filtering.</p>" for i in range(1, 30)]) + """
</body>
</html>
"""

HEAVY_DOM_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Benchmark: Heavy DOM</title>
    <style>
        body { font-family: sans-serif; background: #0f172a; color: #f8fafc; padding: 1rem; }
        .grid { display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr)); gap: 1rem; }
        .card { background: #1e293b; border-radius: 8px; padding: 1rem; border: 1px solid #334155; }
        svg { width: 48px; height: 48px; fill: #4ade80; }
    </style>
</head>
<body>
    <h1>Shadhin Browser Benchmark: Heavy DOM & SVGs</h1>
    <div class="grid">
""" + "\n".join([f"""        <div class="card" id="card-{i}">
            <svg viewBox="0 0 24 24"><path d="M12 2L2 7l10 5 10-5-10-5zM2 17l10 5 10-5M2 12l10 5 10-5"/></svg>
            <h3>Element Card #{i}</h3>
            <p>Complex DOM node with CSS grid, custom SVG vector graphic, and styling rules.</p>
            <span>Node attribute metric: {i * 42}</span>
        </div>""" for i in range(1, 120)]) + """
    </div>
</body>
</html>
"""

TIMER_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Benchmark: JavaScript Timer</title>
    <style>body { font-family: monospace; padding: 2rem; background: #18181b; color: #22c55e; }</style>
</head>
<body>
    <h1>Active JS Timer Scenario</h1>
    <p>Interval tick count: <span id="counter">0</span></p>
    <div id="log" style="height: 200px; overflow-y: scroll; border: 1px solid #27272a; padding: 0.5rem;"></div>
    <script>
        let count = 0;
        const counterEl = document.getElementById('counter');
        const logEl = document.getElementById('log');
        setInterval(() => {
            count++;
            counterEl.innerText = count;
            const p = document.createElement('div');
            p.innerText = '[' + new Date().toISOString() + '] Tick ' + count;
            logEl.appendChild(p);
            if (logEl.children.length > 50) {
                logEl.removeChild(logEl.firstChild);
            }
        }, 100);
    </script>
</body>
</html>
"""

ANIMATION_HTML = """<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <title>Benchmark: Continuous CSS Animation</title>
    <style>
        body { background: #09090b; display: flex; justify-content: center; align-items: center; height: 100vh; margin: 0; }
        .spinner {
            width: 120px;
            height: 120px;
            border: 8px solid rgba(74, 222, 128, 0.2);
            border-top-color: #4ade80;
            border-radius: 50%;
            animation: spin 1s linear infinite;
        }
        @keyframes spin {
            0% { transform: rotate(0deg); }
            100% { transform: rotate(360deg); }
        }
    </style>
</head>
<body>
    <div class="spinner"></div>
</body>
</html>
"""

INDEX_HTML = """<!DOCTYPE html>
<html>
<head><title>Shadhin Benchmark Hub</title></head>
<body>
    <h1>Shadhin Browser Benchmark Test Server</h1>
    <ul>
        <li><a href="/text.html">Scenario A: Plain Text Page (/text.html)</a></li>
        <li><a href="/heavy_dom.html">Scenario B: Heavy DOM Grid (/heavy_dom.html)</a></li>
        <li><a href="/timer.html">Scenario C: JavaScript Timer Loop (/timer.html)</a></li>
        <li><a href="/animation.html">Scenario D: CSS Continuous Animation (/animation.html)</a></li>
        <li><a href="/download.bin">Scenario E: Binary Download (5MB)</a></li>
    </ul>
</body>
</html>
"""

class BenchmarkHandler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path in ('/', '/index.html'):
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            body = INDEX_HTML.encode('utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == '/text.html':
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            body = TEXT_HTML.encode('utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == '/heavy_dom.html':
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            body = HEAVY_DOM_HTML.encode('utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == '/timer.html':
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            body = TIMER_HTML.encode('utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == '/animation.html':
            self.send_response(200)
            self.send_header('Content-Type', 'text/html; charset=utf-8')
            body = ANIMATION_HTML.encode('utf-8')
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        elif self.path == '/download.bin':
            size = 5 * 1024 * 1024 # 5 MB
            self.send_response(200)
            self.send_header('Content-Type', 'application/octet-stream')
            self.send_header('Content-Disposition', 'attachment; filename="benchmark_payload.bin"')
            self.send_header('Content-Length', str(size))
            self.end_headers()
            chunk = b'X' * 65536
            for _ in range(size // len(chunk)):
                self.wfile.write(chunk)
        elif self.path == '/api/status':
            self.send_response(200)
            self.send_header('Content-Type', 'application/json')
            body = b'{"status": "ok"}'
            self.send_header('Content-Length', str(len(body)))
            self.end_headers()
            self.wfile.write(body)
        else:
            self.send_response(404)
            self.end_headers()
            self.wfile.write(b'Not Found')

    def log_message(self, format, *args):
        # Suppress noisy request logging during automated benchmarks
        pass

def main():
    parser = argparse.ArgumentParser(description="Shadhin Benchmark HTTP Server")
    parser.add_argument("--port", type=int, default=8089, help="Port to bind (default: 8089)")
    args = parser.parse_args()

    server_address = ('127.0.0.1', args.port)
    httpd = socketserver.TCPServer(server_address, BenchmarkHandler)
    httpd.allow_reuse_address = True
    print(f"Benchmark server active on http://127.0.0.1:{args.port}")
    try:
        httpd.serve_forever()
    except KeyboardInterrupt:
        httpd.server_close()
        print("\nBenchmark server stopped.")

if __name__ == '__main__':
    main()
