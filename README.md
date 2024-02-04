# StorgataDB

## Build

```sh
./build.sh
```

## Run

### Running in kubernetes standalone

```sh
kubectl apply -f kv.yaml
```

## Cli

StorgataDB is compatible with redis-cli.

```sh
redis-cli -p 30000
```

### Running in kubernetes with other services

```sh
kubctl apply -f db-service.yaml
```
