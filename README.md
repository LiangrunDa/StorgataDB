# StorgataDB

## Build

```sh
./build.sh
```

## Run

### Running in kubernetes

```sh
kubectl apply -f kv.yaml
```

## Cli

StorgataDB is compatible with redis-cli.

```sh
redis-cli -p 30000
```