// End-to-end check of the service on a free port, with Mojang faked (OTERIA_PRESENCE_FAKE_MOJANG).
'use strict';
const { spawn } = require('child_process');
const assert = require('assert');
const crypto = require('crypto');
const fs = require('fs');
const os = require('os');
const path = require('path');

const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'oteria-presence-'));
fs.copyFileSync(path.join(__dirname, 'index.js'), path.join(dir, 'index.js'));
const port = 20000 + Math.floor(Math.random() * 20000);
const server = spawn(process.execPath, [path.join(dir, 'index.js')], {
  env: { ...process.env, PORT: String(port), OTERIA_PRESENCE_FAKE_MOJANG: '1' },
  stdio: ['ignore', 'pipe', 'inherit'],
});
const base = `http://127.0.0.1:${port}`;
const call = async (method, p, body, session) => {
  const r = await fetch(base + p, {
    method,
    headers: { 'Content-Type': 'application/json', ...(session ? { Authorization: `Bearer ${session}` } : {}) },
    body: method === 'GET' ? undefined : JSON.stringify(body ?? {}),
  });
  return { status: r.status, json: await r.json() };
};
const uuid = (name) => crypto.createHash('md5').update(name).digest('hex');

(async () => {
  await new Promise((ok) => server.stdout.once('data', ok));
  const signIn = async (name) => {
    const start = await call('POST', '/v1/auth/start');
    const finish = await call('POST', '/v1/auth/finish', { server_id: start.json.server_id, name });
    assert.equal(finish.status, 200);
    return finish.json.session;
  };
  // A server id is good once, and only one the service gave.
  assert.equal((await call('POST', '/v1/auth/finish', { server_id: 'made-up', name: 'Bob' })).status, 401);
  assert.equal((await call('POST', '/v1/presence', { listed: true })).status, 401);

  const alice = await signIn('Alice');
  assert.deepEqual((await call('POST', '/v1/presence', { listed: true }, alice)).json, { listed: true });
  const asked = [uuid('Alice'), uuid('Bob'), 'not-a-uuid'];
  let worn = (await call('POST', '/v1/worn', { players: asked })).json;
  assert.deepEqual(worn.horizon, [uuid('Alice')]);
  assert.equal(worn.ttl, 300);
  assert.deepEqual(worn.players, {});

  // Not listed any more: forgotten at once.
  await call('POST', '/v1/presence', { listed: false }, alice);
  worn = (await call('POST', '/v1/worn', { players: asked })).json;
  assert.deepEqual(worn.horizon, []);

  // What the game also asks at start.
  assert.deepEqual((await call('GET', '/v1/me', null, alice)).json.owned, []);
  assert.deepEqual((await call('POST', '/v1/emotes', { players: asked })).json, { playing: {} });
  assert.equal((await call('GET', '/nothing')).status, 404);
  console.log('oteria-presence: all checks passed');
})()
  .catch((e) => {
    console.error(e);
    process.exitCode = 1;
  })
  .finally(() => {
    server.kill();
    fs.rmSync(dir, { recursive: true, force: true });
  });
