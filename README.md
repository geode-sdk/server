# Geode Server

Server for the Geode Index, the API used by Geode SDK to retrieve mod information. Based on actix-web and sqlx.

## API Documentation

The API documentation can be found [here](https://api.geode-sdk.org/swagger/). A machine readable openapi.json specification can be found [here](https://api.geode-sdk.org/swagger/openapi.json).

## Running the server

Requires PostgreSQL 17 or later. Two ways to run the server:

- **Docker** (recommended for hosting): see [Running with Docker](#running-with-docker) below. No Rust toolchain needed.
- **From source** (recommended for development): requires the Rust stable toolchain. See the [development environment setup](docs/dev_setup.md) to get started, or just run:
  ```bash
  cargo build # or cargo build --release
  ```

## Running with Docker

Images are published to GHCR with two variants per version, differing only in base OS/libc:

- `-alpine`: musl, statically linked, smaller image (recommended default)
- `-trixie`: glibc, Debian-based

Available as `latest-alpine`/`latest-trixie`, or pinned to a version, e.g. `0.50.0-alpine`/`0.50.0-trixie`.

```bash
docker run -p 3000:3000 \
  -e DATABASE_URL=postgres://user:password@host/schema \
  -e PORT=3000 \
  ghcr.io/geode-sdk/server:latest-alpine
```

Database migrations run automatically on startup, no separate migration step needed.

The image has a built-in `HEALTHCHECK` that polls `GET /` (expects `200`); the same endpoint works for an external load balancer/orchestrator health check if you're not relying on Docker's own.

### Environment variables

| Variable | Required | Default | Description |
| --- | --- | --- | --- |
| `DATABASE_URL` | yes | - | PostgreSQL connection string: `postgres://{username}:{password}@{host}/{database}` |
| `DATABASE_CONNECTIONS` | no | `10` | Max size of the Postgres connection pool |
| `PORT` | no | `8080` | Port the server listens on |
| `APP_DEBUG` | no | `0` | Set to `1` to run single-threaded, for step debugging |
| `APP_URL` | no | `http://localhost:3000` | Used for URL generation in downloads |
| `FRONT_URL` | no | `http://localhost:5173` | Used for link generation (mod status badges, GitHub auth redirects) |
| `GITHUB_CLIENT_ID` | no | - | GitHub OAuth app client ID, used for login |
| `GITHUB_CLIENT_SECRET` | no | - | GitHub OAuth app client secret |
| `DISCORD_WEBHOOK_URL` | no | - | Webhook URL for general index notifications |
| `INDEX_ADMIN_DISCORD_WEBHOOK_URL` | no | - | Webhook URL for admin-only notifications |
| `MAX_MOD_FILESIZE_MB` | no | `250` | Max accepted mod upload size, in MB |
| `DISABLE_DOWNLOAD_COUNTS` | no | `0` | Set to `1` to globally disable download counting (e.g. during abuse) |

### Persisting uploaded files

The server writes uploaded files to `/app/storage` inside the container (public downloads, private files). This is **not** persisted by default, if you remove/recreate the container without a volume, uploads are lost.

Static assets (`/app/static`) are baked into the image itself and don't need persisting, they are re-synced to whatever serves them whenever the image updates, see the Compose example below.

> [!IMPORTANT]
> The index does not serve these files itself in production. You are responsible for serving them with your own reverse proxy: `/app/static` must be served at `/static/`, and `/app/storage/public` at `/storage/`, both relative to `APP_URL`.

The container runs as a fixed non-root user, UID `1000`, GID `1000`.

#### Production example: app + nginx via Compose

A common production setup is to run the app alongside an nginx container sharing the storage volume, with nginx serving `/static` and `/storage` directly and proxying everything else through. This also sidesteps file permission issues you'd otherwise hit trying to read the app's (UID `1000`-owned) storage volume from a host-level reverse proxy.

See [`docs/examples/docker-compose.example.yml`](docs/examples/docker-compose.example.yml) and [`docs/examples/nginx.example.conf`](docs/examples/nginx.example.conf). Your host-level reverse proxy (if any) just forwards to nginx's published port.

#### Other options

**Named volume**: the directory ownership is baked into the image, so this works with no extra setup:

```bash
docker volume create geode-storage

docker run -p 3000:3000 \
  -e DATABASE_URL=postgres://user:password@host/schema \
  -e PORT=3000 \
  -v geode-storage:/app/storage \
  ghcr.io/geode-sdk/server:latest-alpine
```

**Bind mount**: the host directory must be owned by UID/GID `1000` yourself, since Docker doesn't touch bind-mounted host paths' permissions:

```bash
mkdir -p ./geode-storage
chown -R 1000:1000 ./geode-storage

docker run -p 3000:3000 \
  -e DATABASE_URL=postgres://user:password@host/schema \
  -e PORT=3000 \
  -v ./geode-storage:/app/storage \
  ghcr.io/geode-sdk/server:latest-alpine
```

If your host already has a user/group with UID/GID `1000` (common for the first non-root user on Linux), you may already own the directory by default and can skip the `chown`.
