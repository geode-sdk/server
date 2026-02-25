# Geode Server

Server for the Geode Index, the API used by Geode SDK to retrieve mod information. Based on actix-web and sqlx.

## Requirements for hosting

- rust stable
- PostgreSQL 14 or later

## Documentation

The API documentation can be found [here](https://api.geode-sdk.org/swagger/). A machine readable openapi.json specification can be found [here](https://api.geode-sdk.org/swagger/openapi.json).

## Configuration

Check out the [development environment setup](docs/dev_setup.md) to get started!

## Building the server

```bash
cargo build # or cargo build --release
```
