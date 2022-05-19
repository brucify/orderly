
# orderly

Pulls order depths for the given currency pair from the WebSocket feeds of multiple exchanges.
Publishes a merged order book as a gRPC stream.

Currently supports: 

* Bitstamp WebSocket: `wss://ws.bitstamp.net`
* Binance WebSocket: `wss://stream.binance.com:9443/ws`
* Kraken WebSocket: `wss://ws.kraken.com`

```
USAGE:
    orderly-server [OPTIONS]

OPTIONS:
    -h, --help               Print help information
        --no-binance         (Optional) Don't show Binance in gRPC stream. Default: false
        --no-bitstamp        (Optional) Don't show Bitstamp in gRPC stream. Default: false
        --no-kraken          (Optional) Don't show Kraken in gRPC stream. Default: false
    -p, --port <PORT>        (Optional) Port number on which the the gRPC server will be hosted.
                             Default: 50051
    -s, --symbol <SYMBOL>    (Optional) Currency pair to subscribe to. Default: ETH/BTC
```

Run gRPC server:

```
cargo run --bin orderly-server
```
or with logs and options:
```
env RUST_LOG=info cargo run --bin orderly-server -- --symbol ETH/BTC --port 50051
```
Exclude certain exchanges:

```
cargo run --bin orderly-server -- --no-binance --no-bitstamp
```


<img src="https://user-images.githubusercontent.com/1086619/169376599-7eccdb75-08ad-4273-a18a-197ebd53dd63.jpg" width="700"/>

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

