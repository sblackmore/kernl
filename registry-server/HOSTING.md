# Hosting `kernl-registry`

The registry is a small HTTP server (publish, download, search). Run it behind TLS and authentication in production.

## Docker Compose

From `registry-server/`:

```bash
docker compose up --build -d
```

Default URL: `http://127.0.0.1:9955`

Point the package CLI at it:

```bash
export KERNL_REGISTRY_URL=http://127.0.0.1:9955/api/v1
kernl publish
kernl search mypkg
```

## Environment

| Variable | Meaning |
|----------|---------|
| `KERNL_REGISTRY_PORT` | TCP port (default `3400`; Docker image sets `9955`) |
| `KERNL_REGISTRY_DATA` | Directory for stored packages (default `./data`) |

CLI flags `--port`, `--data-dir`, and `--rate-limit` override env defaults when passed.

See `src/config.rs` for defaults and `src/auth.rs` for optional publish tokens.

## Production checklist

- Terminate TLS at a reverse proxy (Caddy, nginx, cloud load balancer).
- Set a shared secret via registry auth config and distribute to publishers only.
- Back up the volume backing `KERNL_REGISTRY_DATA`.
