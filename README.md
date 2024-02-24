# StorgataDB

StorgataDB is a distributed key-value store built on top of [Raft Lite](https://github.com/LiangrunDa/raft-lite/tree/main). It is compatible with RESP (REdis Serialization Protocol).

## Build

```sh
./build.sh
```

## Run

### Running in kubernetes standalone

```sh
kubectl apply -f test-kv.yaml
```

### Running in kubernetes

```sh
kubctl apply -f db-service.yaml
```

## Cli

StorgataDB is compatible with redis-cli.

```sh
redis-cli -p 30000
```
