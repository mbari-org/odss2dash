## odss2dash

General overview at: https://docs.mbari.org/tethysdash/odss2dash/.

`odss2dash` is a service that periodically retrieves the latest positions
of specified assets from the Tracking DB (via ODSS API)
to then relay them to any number of configured TethysDash instances
(which in turn will push them to corresponding Dash UI instances via websockets).

- Configuration is captured in a local `odss2dash.toml` file.
- Multiple TethysDash instances can be configured, to which
  any new positions will be notified.
  We use this mechanism for the production TethysDash instance
  (visible through `okeanids`)
  as well as for instances running on our TethysDash staging server, `tethystest`.
- Each TethysDash instance configuration includes a corresponding API Key,
  which `odss2dash` uses to be able to make the relevant notification requests.
- The service will perform the following dispatch repeatedly according to
  the `pollPeriod` configuration setting:
  - Read in the desired assets to be dispatched from `./dispatched.json`
  - Retrieve any newer positions of those platforms from TrackingDB/ODSS
  - Notify the new positions to the configured TethysDash instances
  - Update `./reported.json`, which keeps track of the timestamp of the latest
    reported position for each dispatched platform.

The REST API allows clients to update the list of assets to be dispatched.
The Dash UI, in particular, uses it to populate the TrackingDB platforms dropdown
where the user can select the platforms to be included on the map.

### Configuration

See `odss2dash.toml` for details.
In particular, make sure to use the `$EV` mechanism to indicate the TethysDash API Keys.
Set the corresponding environment variables accordingly, prior to running the program.
You can put them in a `.env` file, which is automatically ingested.

### Running

The primary command of the program is to launch the service itself:
```shell
odss2dash serve
```

Locally, you can then open <http://localhost:3033/apidoc>.

`odss2dash serve` is the command associated to the docker image.

> Mainly for development/verification purposes,
> other commands are also available.
> Run `odss2dash --help` for more details.

### Okeanids

Upon a new pushed git tag, `odss2dash` gets automatically updated on `okeanids`.
Along with other TethysDash system components, it is included in our regular
continuous deployment (CD) setup. 

The `odss2dash` API is exposed at `https://okeanids.mbari.org/odss2dash/api`,
with documentation available at <https://okeanids.mbari.org/odss2dash/apidoc/>.
