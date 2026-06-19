// interface/http-server.mjs
// HTTP presentation layer (Node `node:http`, zero third-party deps).
//
// Clean Architecture: this layer only calls the injected `service` (application).
// It holds NO business rules — it parses HTTP, dispatches, and serializes
// responses. The module exports a factory that returns an unstarted
// `http.Server`; the caller/test owns `.listen()` (e.g. `.listen(0)`).
//
// Routes:
//   POST /shorten  body {"url":"..."} -> 201 {code, short:"/"+code}
//                  bad body / missing url / service throw -> 400 {error}
//   GET  /:code    -> 302 Location: <url> on hit; 404 on unknown code

import { createServer as createHttpServer } from 'node:http';

const MAX_BODY_BYTES = 1 << 20; // 1 MiB cap — reject oversized request bodies.

// Sentinel thrown by readBody once it has already written a 413 response, so the
// caller skips its own error serialization instead of double-writing the head.
const BODY_TOO_LARGE = Symbol('body too large');

/**
 * Send a JSON response with the given status code.
 * @param {import('node:http').ServerResponse} res
 * @param {number} status
 * @param {unknown} payload
 * @returns {void}
 */
function sendJson(res, status, payload) {
  const body = JSON.stringify(payload);
  res.writeHead(status, { 'content-type': 'application/json; charset=utf-8' });
  res.end(body);
}

/**
 * Read the full request body as a UTF-8 string. On exceeding the cap, send a
 * graceful 413 (instead of resetting the connection) and reject with the
 * BODY_TOO_LARGE sentinel so the caller knows the response is already written.
 * @param {import('node:http').IncomingMessage} req
 * @param {import('node:http').ServerResponse} res
 * @returns {Promise<string>}
 */
function readBody(req, res) {
  return new Promise((resolve, reject) => {
    const chunks = [];
    let size = 0;
    req.on('data', (chunk) => {
      size += chunk.length;
      if (size > MAX_BODY_BYTES) {
        sendJson(res, 413, { error: 'request body too large' });
        reject(BODY_TOO_LARGE);
        req.destroy(); // stop reading; response already sent above.
        return;
      }
      chunks.push(chunk);
    });
    req.on('end', () => resolve(Buffer.concat(chunks).toString('utf8')));
    req.on('error', reject);
  });
}

/**
 * Parse a JSON object body and extract a `url` string field.
 * @param {string} raw  raw request body
 * @returns {string} the `url` value
 * @throws {Error} on invalid JSON or a missing/non-string `url`
 */
function parseUrlFromBody(raw) {
  let parsed;
  try {
    parsed = JSON.parse(raw);
  } catch {
    throw new Error('invalid JSON body');
  }
  if (!parsed || typeof parsed !== 'object' || typeof parsed.url !== 'string') {
    throw new Error('missing or invalid "url" field');
  }
  return parsed.url;
}

/**
 * Handle POST /shorten: parse body, call service, return 201 or 400.
 * @param {import('node:http').IncomingMessage} req
 * @param {import('node:http').ServerResponse} res
 * @param {{ shorten: (url: string) => { code: string } }} service
 * @returns {Promise<void>}
 */
async function handleShorten(req, res, service) {
  try {
    const raw = await readBody(req, res);
    const url = parseUrlFromBody(raw);
    const { code } = service.shorten(url);
    sendJson(res, 201, { code, short: `/${code}` });
  } catch (err) {
    if (err === BODY_TOO_LARGE) return; // 413 already sent by readBody.
    sendJson(res, 400, { error: err.message });
  }
}

/**
 * Handle GET /:code: resolve via service, 302 redirect on hit else 404.
 * @param {import('node:http').ServerResponse} res
 * @param {string} code  path segment (already stripped of leading '/')
 * @param {{ resolve: (code: string) => (string | undefined) }} service
 * @returns {void}
 */
function handleResolve(res, code, service) {
  const url = service.resolve(code);
  if (url === undefined) {
    sendJson(res, 404, { error: 'unknown code' });
    return;
  }
  res.writeHead(302, { location: url });
  res.end();
}

/**
 * Route a single request to the right handler.
 * @param {import('node:http').IncomingMessage} req
 * @param {import('node:http').ServerResponse} res
 * @param {object} service
 * @returns {void | Promise<void>}
 */
function route(req, res, service) {
  let pathname;
  try {
    ({ pathname } = new URL(req.url, 'http://localhost'));
  } catch {
    sendJson(res, 400, { error: 'bad request' });
    return undefined;
  }
  if (req.method === 'POST' && pathname === '/shorten') {
    return handleShorten(req, res, service);
  }
  const code = pathname.slice(1); // strip leading '/'
  if (req.method === 'GET' && code.length > 0 && !code.includes('/')) {
    return handleResolve(res, code, service);
  }
  sendJson(res, 404, { error: 'not found' });
  return undefined;
}

/**
 * Build an unstarted HTTP server bound to the given application service.
 * The caller is responsible for `.listen()` (tests use `.listen(0)`).
 *
 * @param {{
 *   shorten: (url: string) => { code: string },
 *   resolve: (code: string) => (string | undefined),
 * }} service  application service (injected; the only collaborator).
 * @returns {import('node:http').Server}
 */
export function createServer(service) {
  if (!service || typeof service.shorten !== 'function'
    || typeof service.resolve !== 'function') {
    throw new Error('createServer: a service { shorten, resolve } is required');
  }
  // Guard every dispatch: a synchronous throw or a rejected handler promise
  // must never escape and crash the server. The error handler only writes when
  // the response head has not already been sent (avoids double-write crashes).
  const onError = (res) => {
    if (!res.headersSent) sendJson(res, 400, { error: 'bad request' });
  };
  return createHttpServer((req, res) => {
    try {
      Promise.resolve(route(req, res, service)).catch(() => onError(res));
    } catch {
      onError(res);
    }
  });
}
