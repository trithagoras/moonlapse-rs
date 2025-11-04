# Moonlapse

## Building and running

### Server

**(DB setup coming soon)**

From the root directory, simply run:
```sh
cargo run -p moonlapse-server
```

This will build both the shared library `moonlapse-shared` and start the server on port 42523.

If you want to choose a specific port number, you can pass in the port argument via:

```sh
cargo run -p moonlapse-server -- <port>
```

Choosing port '0' will allow the OS to select an unused port. This will be logged to you when starting the server.

### Client

**(coming soon)**

From the root directory, simply run:
```sh
cargo run -p moonlapse-client
```

This will build both the shared library `moonlapse-shared` and start the client.