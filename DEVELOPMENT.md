# Development

Routine command recipes are captured in [`justfile`](justfile),
to be run with the handy [`just`](https://github.com/casey/j) tool,
which I alias to `j` in my shell.

Prior to committing/pushing changes, I run `j all`, which includes
testing, formatting, and linting.

To see the help message from the `odss2dash` program:
```shell
j run --help
```
```
cargo run -- --help
    Finished dev [unoptimized + debuginfo] target(s) in 0.14s
     Running `target/debug/odss2dash --help`
odss2dash:
     The central function of this program is a service that relays
     platform positions from the TrackingDB/ODSS to TethysDash.

Usage: odss2dash <COMMAND>

Commands:
  check-config    Perform basic configuration checks
  get-platforms   Get all platforms from TrackingDB/ODSS
  get-platform    Get platform information from TrackingDB/ODSS
  get-positions   Get platform positions from TrackingDB/ODSS
  add-dispatched  Add platforms to be dispatched
  dispatch        Launch dispatch according to configuration
  serve           Launch service
  health          Get health similar to the endpoint
  help            Print this message or the help of the given subcommand(s)

Options:
  -h, --help
          Print help (see a summary with '-h')

  -V, --version
          Print version
```

For convenience, various recipes launch specific program commands:
```shell
j serve --no-dispatch
```
```
cargo run -- serve --no-dispatch
...
Platform cache initialized with 292 platforms found in TrackingDB/ODSS
Loading './dispatched.json'
api : http://localhost:3033/api
doc : http://localhost:3033/apidoc/
spec: http://localhost:3033/api-docs/openapi.json
Server listening on 0.0.0.0:3033
```

Run `j` to see the list of recipes.
These include running the certain functions directly
(mainly for quick verification purposes), or via docker, as well as
making requests (using `curlie`) to the local service. 

Of particular interest, but still mainly for local validation(*)

- `j dockerize`: creates docker image
- `j docker-run serve --no-dispatch`: runs the image

> (*) A GitHub Action is in place to automatically build the image
> and trigger continuous deployment on production server.


## About v2

v2 is a reimplementation of the odss2dash system in Rust.
Without being trivial, the odss2dash service is simple enough to further 
evaluate the Rust ecosystem for development of web services.
Execution of the new version on the production system has been very satisfactory.
As with other TethysDash components, this now also includes continuous deployment:
just pushing a new tag to the repo will trigger an automatic image build and subsequent
update of the running container.

- [x] Initial version with configuration handling
- [x] Direct requests to the Tracking DB
- [x] Dispatch of position polling and notification to configured TethysDash instances
- [x] Service with REST API and OpenAPI documentation
- [x] Dockerization
- [x] Testing

(The v1 code is [here](https://github.com/mbari-org/odss2dash1).)
