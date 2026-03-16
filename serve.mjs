import http from 'node:http';
import { createReadStream, existsSync, statSync } from 'node:fs';
import { extname, join, normalize, resolve } from 'node:path';

const port = Number(process.env.PORT || 8787);
const root = resolve('.');

const mimeTypes = {
  '.html': 'text/html; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.jpg': 'image/jpeg',
  '.jpeg': 'image/jpeg',
  '.gif': 'image/gif',
  '.webp': 'image/webp',
  '.ico': 'image/x-icon',
  '.wasm': 'application/wasm',
  '.txt': 'text/plain; charset=utf-8'
};

function send(res, statusCode, body, contentType = 'text/plain; charset=utf-8') {
  res.writeHead(statusCode, {
    'Content-Type': contentType,
    'Cross-Origin-Opener-Policy': 'same-origin',
    'Cross-Origin-Embedder-Policy': 'require-corp',
    'Cross-Origin-Resource-Policy': 'same-origin',
    'Cache-Control': 'no-store'
  });
  res.end(body);
}

const server = http.createServer((req, res) => {
  try {
    const url = new URL(req.url || '/', `http://${req.headers.host || 'localhost'}`);
    let pathname = decodeURIComponent(url.pathname);

    if (pathname === '/') pathname = '/index.html';

    const safePath = normalize(pathname).replace(/^\.\.(\/|\\|$)+/, '');
    const filePath = resolve(join(root, `.${safePath}`));

    if (!filePath.startsWith(root)) {
      send(res, 403, 'Forbidden');
      return;
    }

    if (!existsSync(filePath)) {
      send(res, 404, 'Not found');
      return;
    }

    const stats = statSync(filePath);
    if (stats.isDirectory()) {
      send(res, 403, 'Directory listing is disabled');
      return;
    }

    const contentType = mimeTypes[extname(filePath).toLowerCase()] || 'application/octet-stream';
    res.writeHead(200, {
      'Content-Type': contentType,
      'Content-Length': stats.size,
      'Cross-Origin-Opener-Policy': 'same-origin',
      'Cross-Origin-Embedder-Policy': 'require-corp',
      'Cross-Origin-Resource-Policy': 'same-origin',
      'Cache-Control': 'no-store'
    });

    createReadStream(filePath).pipe(res);
  } catch (error) {
    send(res, 500, `Server error: ${error.message}`);
  }
});

server.on('error', (error) => {
  console.error(`Failed to start server on http://127.0.0.1:${port}/`);
  console.error(error.message);
  process.exit(1);
});

server.listen(port, '127.0.0.1', () => {
  console.log(`Serving ${root}`);
  console.log(`Open http://127.0.0.1:${port}/`);
  console.log('COOP/COEP headers enabled for SharedArrayBuffer');
});
