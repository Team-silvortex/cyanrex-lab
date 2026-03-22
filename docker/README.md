# docker

Run the full Cyanrex stack with Docker Compose.

## Services

- `postgres` on `localhost:5432`
- `engine` on `localhost:8080`
- `frontend` on `localhost:3000`

## eBPF Notes

- `engine` service is configured with elevated Linux capabilities for eBPF loading.
- `memlock` is set to unlimited in compose to satisfy libbpf requirements.
- Host paths `/sys/fs/bpf` and `/lib/modules` are mounted for kernel/BPF integration.

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
