// Oteria's presence service: which players play Oteria Client, so each client can draw Oteria's
// badge by their names. It speaks the part of the cosmetics service's protocol the game uses
// (crates/horizon-cosmetics: account.rs, fetch.rs, feed.rs), with no cosmetics of its own:
//
//   POST /v1/auth/start   {}                          -> {"server_id"}
//   POST /v1/auth/finish  {"server_id", "name"}       -> {"session", "expires_in"}
//        The game told Mojang it joins `server_id` (as when joining a server); Mojang's
//        hasJoined confirms it, so nobody can list someone else.
//   POST /v1/presence     {"listed": bool}  (Bearer)  -> {"listed"}
//        Listed: reported for PRESENCE_WINDOW after the last time (the game says it again
//        every hour). Not listed: forgotten at once.
//   POST /v1/worn         {"players": [uuid hex…]}    -> {"ttl", "players": {}, "horizon": [listed uuids]}
//   GET  /v1/me, PUT /v1/visible, POST /v1/emote, POST /v1/emotes: empty answers (no cosmetics).
//
// No dependencies: Node 18 or later (fetch). The port is SERVER_PORT (game panels), else PORT,
// else 8080. Who is listed is kept in presence.json beside this file, so a restart forgets nobody.

'use strict';
const http = require('http');
const crypto = require('crypto');
const fs = require('fs');
const path = require('path');

const PORT = Number(process.env.SERVER_PORT || process.env.PORT || 8080);
const PRESENCE_WINDOW = 3 * 3600 * 1000;
const SESSION_LIFE = 3600 * 1000;
const SERVER_ID_LIFE = 2 * 60 * 1000;
const WORN_TTL = 300;
const BATCH = 64;
const MAX_BODY = 64 * 1024;
const STORE = path.join(__dirname, 'presence.json');

/** uuid hex -> last time it said "I play Oteria" (ms). */
const listed = new Map();
/** session token -> { uuid, until } */
const sessions = new Map();
/** server id -> until */
const serverIds = new Map();

function load() {
  try {
    const saved = JSON.parse(fs.readFileSync(STORE, 'utf8'));
    for (const [uuid, at] of Object.entries(saved)) {
      if (Date.now() - at < PRESENCE_WINDOW) listed.set(uuid, at);
    }
  } catch (_) {
    // No file yet, or a broken one: start empty.
  }
}

function save() {
  const out = {};
  for (const [uuid, at] of listed) out[uuid] = at;
  fs.writeFile(STORE + '.tmp', JSON.stringify(out), (err) => {
    if (!err) fs.rename(STORE + '.tmp', STORE, () => {});
  });
}

/** Drops what has expired. */
function sweep() {
  const now = Date.now();
  for (const [uuid, at] of listed) if (now - at >= PRESENCE_WINDOW) listed.delete(uuid);
  for (const [s, v] of sessions) if (now >= v.until) sessions.delete(s);
  for (const [id, until] of serverIds) if (now >= until) serverIds.delete(id);
}

/** The last requests' summaries, in trace.log beside this file (read over SFTP to debug). */
const traced = [];
function trace(line) {
  traced.push(`${new Date().toISOString()} ${line}`);
  if (traced.length > 300) traced.splice(0, traced.length - 300);
}
const NL = String.fromCharCode(10);
setInterval(() => {
  fs.writeFile(path.join(__dirname, 'trace.log'), traced.join(NL) + NL, () => {});
}, 5000).unref();

function send(res, status, body) {
  const text = JSON.stringify(body);
  res.writeHead(status, {
    'Content-Type': 'application/json',
    'Content-Length': Buffer.byteLength(text),
  });
  res.end(text);
}

function readBody(req) {
  return new Promise((resolve, reject) => {
    let size = 0;
    const chunks = [];
    req.on('data', (c) => {
      size += c.length;
      if (size > MAX_BODY) {
        reject(new Error('too large'));
        req.destroy();
      } else chunks.push(c);
    });
    req.on('end', () => {
      const text = Buffer.concat(chunks).toString('utf8').trim();
      if (!text || text === 'null') return resolve({});
      try {
        resolve(JSON.parse(text));
      } catch (_) {
        reject(new Error('bad json'));
      }
    });
    req.on('error', reject);
  });
}

/** The account a request's Bearer session belongs to, or null. */
function sessionOf(req) {
  const auth = req.headers['authorization'] || '';
  const token = auth.startsWith('Bearer ') ? auth.slice(7) : '';
  const s = sessions.get(token);
  return s && Date.now() < s.until ? s : null;
}

const isUuid = (v) => typeof v === 'string' && /^[0-9a-f]{32}$/.test(v);

async function handle(req, res) {
  const url = new URL(req.url, 'http://x');
  const route = `${req.method} ${url.pathname}`;
  if (route === 'GET /' || route === 'GET /health') {
    return send(res, 200, { service: 'oteria-presence', listed: listed.size });
  }
  let body;
  try {
    body = req.method === 'GET' ? {} : await readBody(req);
  } catch (e) {
    return send(res, 400, { error: e.message });
  }
  switch (route) {
    case 'POST /v1/auth/start': {
      const id = crypto.randomBytes(16).toString('hex');
      serverIds.set(id, Date.now() + SERVER_ID_LIFE);
      return send(res, 200, { server_id: id });
    }
    case 'POST /v1/auth/finish': {
      const { server_id: id, name } = body;
      if (typeof id !== 'string' || !serverIds.has(id) || typeof name !== 'string' || !/^\w{1,16}$/.test(name)) {
        return send(res, 401, { error: 'unknown server id or name' });
      }
      serverIds.delete(id);
      let profile;
      // Tests only (test.js): every name joins, with an id made from it.
      if (process.env.OTERIA_PRESENCE_FAKE_MOJANG === '1') {
        profile = { id: crypto.createHash('md5').update(name).digest('hex') };
      } else try {
        const answer = await fetch(
          `https://sessionserver.mojang.com/session/minecraft/hasJoined?username=${encodeURIComponent(name)}&serverId=${encodeURIComponent(id)}`,
          { signal: AbortSignal.timeout(10000) },
        );
        if (answer.status !== 200) return send(res, 401, { error: 'not joined' });
        profile = await answer.json();
      } catch (_) {
        return send(res, 502, { error: 'Mojang did not answer' });
      }
      if (!profile || !isUuid(profile.id)) return send(res, 401, { error: 'not joined' });
      const session = crypto.randomBytes(32).toString('hex');
      sessions.set(session, { uuid: profile.id, until: Date.now() + SESSION_LIFE });
      return send(res, 200, { session, expires_in: SESSION_LIFE / 1000 });
    }
    case 'POST /v1/presence': {
      const s = sessionOf(req);
      if (!s) return send(res, 401, { error: 'no session' });
      trace(`presence ${s.uuid} listed=${body.listed === true}`);
      if (body.listed === true) listed.set(s.uuid, Date.now());
      else listed.delete(s.uuid);
      save();
      return send(res, 200, { listed: body.listed === true });
    }
    case 'POST /v1/worn': {
      const players = Array.isArray(body.players) ? body.players.slice(0, BATCH) : [];
      const now = Date.now();
      const oteria = players.filter((u) => isUuid(u) && now - (listed.get(u) || 0) < PRESENCE_WINDOW);
      trace(`worn asked=${players.length} [${players.slice(0, 8).join(',')}] oteria=[${oteria.join(',')}]`);
      return send(res, 200, { ttl: WORN_TTL, players: {}, horizon: oteria });
    }
    case 'GET /v1/me': {
      if (!sessionOf(req)) return send(res, 401, { error: 'no session' });
      return send(res, 200, { owned: [], emotes: [], visible: true });
    }
    case 'PUT /v1/visible': {
      if (!sessionOf(req)) return send(res, 401, { error: 'no session' });
      return send(res, 200, { visible: body.visible === true });
    }
    case 'POST /v1/emote': {
      if (!sessionOf(req)) return send(res, 401, { error: 'no session' });
      return send(res, 200, { emote: null });
    }
    case 'POST /v1/emotes':
      return send(res, 200, { playing: {} });
    default:
      return send(res, 404, { error: 'not found' });
  }
}

load();
// Tests only: these UUIDs count as listed from the start (offline players can't sign in).
for (const uuid of (process.env.OTERIA_PRESENCE_LIST || '').split(',')) {
  if (isUuid(uuid)) listed.set(uuid, Date.now());
}
setInterval(sweep, 60 * 1000).unref();
setInterval(save, 10 * 60 * 1000).unref();
http
  .createServer((req, res) => {
    handle(req, res).catch(() => send(res, 500, { error: 'internal' }));
  })
  .listen(PORT, '0.0.0.0', () => {
    console.log(`oteria-presence listening on port ${PORT} (${listed.size} listed)`);
    // Where it listens, for whoever has only the files (a game panel's SFTP).
    fs.writeFile(
      path.join(__dirname, 'listening.json'),
      JSON.stringify({ port: PORT, started: new Date().toISOString(), node: process.version }),
      () => {},
    );
  });
