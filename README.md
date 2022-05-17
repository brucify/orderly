# orderly

Pulls order depths for the given currency pair from the WebSocket feeds of multiple exchanges.
Publishes a merged order book as a gRPC stream.

Currently supports: 

* Bitstamp WebSocket: `wss://ws.bitstamp.net`
* Binance WebSocket: `wss://stream.binance.com:9443/ws`

```
USAGE:
    orderly-server [OPTIONS]

OPTIONS:
    -h, --help               Print help information
    -p, --port <PORT>        (Optional) Port number on which the the gRPC server will be hosted.
                             Default: 50051
    -s, --symbol <SYMBOL>    (Optional) Currency pair to subscribe to. Default: ethbtc
```

Run gRPC server:

```
cargo run --bin orderly-server
```
or with logs and options:
```
env RUST_LOG=info cargo run --bin orderly-server -- --symbol ethbtc --port 50051
```

<img src="https://user-images.githubusercontent.com/1086619/168685536-b4f244c2-596b-4295-a253-a2382b5e095d.jpg" width="700"/>

Client
-----

Connects to the gRPC server and streams the orderbook summary.

```
USAGE:
    orderly-client [OPTIONS]

OPTIONS:
    -h, --help           Print help information
    -p, --port <PORT>    (Optional) Port number of the gRPC server. Default: 50051
```

Run gRPC client:

```
cargo run --bin orderly-client
```
or with logs and options:

```
env RUST_LOG=info cargo run --bin orderly-client -- --port 50051
```
