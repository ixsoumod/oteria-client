# oteria-presence

Which players play Oteria Client, so every client draws Oteria's badge by their names. It speaks
the part of the cosmetics service's protocol the game uses (`crates/horizon-cosmetics`), with no
cosmetics of its own. Node.js 18 or later, **no npm dependency**.

## Run it

- Files: `index.js` and `package.json` (nothing to `npm install`).
- Start: `node index.js` (or `npm start`). The port is `SERVER_PORT` (game panels such as
  Pterodactyl set it), else `PORT`, else 8080.
- It keeps who is listed in `presence.json` beside it, so a restart forgets nobody.
- `GET /` answers `{"service": "oteria-presence", "listed": N}`: a quick check it runs.

## Point the game at it

`COSMETICS_API` in `crates/horizon-addresses/src/lib.rs` (or `HORIZON_COSMETICS_API` for one
run): `http://<ip>:<port>`, or `https://<domain>` behind a proxy that does TLS.

## Test

`node test.js`: starts it on a free port with Mojang faked (`OTERIA_PRESENCE_FAKE_MOJANG=1`, for
tests only) and checks sign-in, presence and the batched question.
