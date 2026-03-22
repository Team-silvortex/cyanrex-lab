# docker

Run the full Cyanrex stack with Docker Compose.

## Services

- `postgres` on `localhost:5432`
- `engine` on `localhost:8080`
- `frontend` on `localhost:3000`

## Start

```bash
docker compose -f docker/docker-compose.yml up --build -d
```

## Check

```bash
docker compose -f docker/docker-compose.yml ps
curl http://localhost:8080/health
```

## Stop

```bash
docker compose -f docker/docker-compose.yml down
```
