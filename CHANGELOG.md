2026-09

2.5.2

- Print the API key expirations at startup (rather than logging them).
- Report the reason a TethysDash instance refused an XEvent.

2.5.1

- Every positions request to ODSS now carries a `startDate`. The unbounded form, which asks
  for the latest positions with no date at all, is the one ODSS is slowest to answer: some
  platforms take 33 to 60 seconds to report their latest 20 positions that way, against
  0.1s for the same platforms with a date bound. It was also what the dispatcher used for
  every platform on every cycle, which is why four of them were timing out every minute.
    - The dispatcher now asks for the positions since the one it last reported, which it
      already keeps track of, and never looks further back than the new `dispatchCatchUpWindow`
      (1 hour), so that returning from an outage does not replay everything meanwhile.
    - A request for the latest positions with no `endDate` derives its lower bound from
      `defaultPositionsWindow` as well, rather than being left unbounded.
    - Removed `defaultLastNumberOfFixes`: the dispatcher no longer needs a number of
      positions to ask for, and what remained of the setting was the limit applied to a
      request that does not give one, which is now a constant.
    - Removed the fallback that asked for the last known positions of a platform with
      nothing in the window: reporting positions of arbitrary age is not a use case, and it
      was the only remaining unbounded request.

2.5.0

- Add [`notes/odss.md`](notes/odss.md) to summarize how the ODSS endpoints behave and what
  odss2dash sends to them, along with the measurements behind the adjustments below.
  Note that ODSS is being rewritten.
- Keep the positions requests to ODSS bounded
    - A request with `endDate` but no `startDate` is not delimiting a period, but asking for
      the latest positions as of that moment. It made ODSS return the complete track of the
      platform: 32MB for D_PACKARD, 128MB for W_FLYER, over the 30MB limit and the timeout,
      so it ended up as a 404. A lower bound is now derived from the new `defaultPositionsWindow`
      setting, falling back to a request for the latest positions if that window captures nothing.
    - When the request delimits an explicit period, `numberOfPoints` is now passed to ODSS,
      which strides the track over the period, returning the first and last position plus
      every Nth one in between, instead of transferring every position only to truncate the
      response here.
      A one-year period of D_PACKARD went from 23.6MB (of which only the newest 1001 positions,
      some 17 hours, were kept) to 96KB covering the whole year.
    - A period is also requested with `taperAccuracy`, so that its most recent third of the
      points comes back position by position and only the older part is strided. A period
      holding no more positions than requested is not strided at all, ODSS leaving it alone
      in that case, so no threshold on the length of the period is involved.
    - New `odssTimeout` setting, defaulting to 10s, that is, below the Dash4 client timeout.
- A failed request to ODSS is now reported as 504 (timeout) or 502, instead of as a 404
  indistinguishable from the platform not existing.

- Use `reqwest` instead of `ureq` for the outgoing requests, so that the ODSS calls
  no longer block a tokio worker thread. A slow ODSS response used to stall every
  other request, including the ones served from the platform cache.
  A single, shared client and runtime are now used process-wide,
  with connections (and their TLS handshakes) reused across requests.
- The platform cache lock is no longer held while waiting on ODSS.
- Add compression.
- Separate config file for test_parse_config_file
- Update some dependencies.

- 2.4.9

- Log token expiration at publisher construction and show it in `check-config` command.
- Dependency updates.

2026-05

2.4.8

- Add /metrics endpoint.
- Update some dependencies.

2026-04

2.4.7

- Maintenance update (<https://github.com/mbari-org/odss2dash/pull/12>).

2026-02

2.4.6

- Maintenance update (fix to https://github.com/mbari-org/odss2dash/security/dependabot/12).

2026-01

2.4.5

- Incremented limit to parse JSON response in trackdb_client.
  Multiple errors in the log were exposed today with platform `669811c71c6e31522c529a7f` (D_Packard),
  indicating that the (default) 10MB limit was exceeded, and these rendered the service unusable.
  As a quick solution, incremented limit to 30MB.
  

2025-03

2.4.1

- Maintenance update (dependabot fix).

2.4.0

- Refactored openapi related routers, and added rapidoc
- Internal: move create_swagger_router to a module

2.3.5

- Maintenance update

2025-02

2.3.4

- use ureq
- some code cleanup

2.3.3

- Maintenance update

2.3.2

- Re-enabled graceful shutdown

2.3.1

- Updated CI/release workflows
- Updated dependencies 

2025-01

2.3.0

- Updated dependencies.
    - should fix recent dependabot issues 
    - also updated axum from 0.7.2 to 0.8.1, which just required changing the path 
      parameter syntax from /:single to /{single} (odss2dash didn't use /*many).

2024-08

2.2.5

- maintenance update

2.2.4

- Issue note in ci with time dependency, which a simple `cargo update` fixes
  (while also bringing in a few other updates)
- dependabot fix incorporated. 

2024-04

2.2.3

- dependabot fix

2024-03

2.2.2

- `cargo update`
- dependabot fix

2024-02

2.2.0

- trackdb_client: properly handle response, e.g., when an invalid platform ID is queried.

2.1.2

- some lib updates 
- added `health` command.
- `j run serve --no-dispatch` is a good way to locally test the utoipa ui.

2024-01

2.1.1

- dependabot fix

2023-12

- added a `/health` endpoint
- updated axum from 0.6 to 0.7 (https://tokio.rs/blog/2023-11-27-announcing-axum-0-7-0)
- `cargo update`

2023-10

- some dependencies updated
- `cargo update`

2023-07

- v2, improved reimplementation of the system.
