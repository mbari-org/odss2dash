# The ODSS API as odss2dash uses it

> [!NOTE]
> ODSS is being rewritten. 
> These notes describe the service deployed at <https://odss.mbari.org/odss> as of Sept 2026.
> Everything below is observed behavior, confirmed by querying that service;
> expect it to change, and recheck before relying on any of it.

## Endpoints

| endpoint | used for | observed |
|---|---|---|
| `GET /platforms` | the platform cache | ~78KB, 0.12-0.28s for the 364 platforms |
| `GET /platforms/{id}` | a single platform | an unknown id answers `200` with a `null` (or empty) body, not a `404` |
| `GET /tracks` | positions | see below |

## `/tracks` parameters

`platformID`, `returnFormat=json` and `returnSRS=4326` go on every request. The rest
control two independent things: which positions are selected, and how many of them are
reported.

### Which positions are selected

- **`lastNumberOfFixes`**, when positive, selects the latest N positions as of now, and the
  dates are discarded. Asking for `lastNumberOfFixes=5` together with an `endDate` a day in
  the past still answers with the five latest positions as of now, with no warning that the
  bound was dropped.
- **`startDate` / `endDate`** select the positions between them, and only when
  `lastNumberOfFixes` is absent. An `endDate` on its own leaves the selection unbounded
  below, so the answer is the platform's complete track. That is 32.7MB in 9.6s for
  `D_PACKARD` and 127.9MB in 46.6s for `W_FLYER`, against 6.8KB for a glider retired in
  2015: the cost depends entirely on how long and how often the platform has been
  reporting.

So the two are alternatives, and `lastNumberOfFixes` wins. A period *with a limit on it* is
not expressible in a single ODSS request.

### How many are reported

These apply to whatever was selected, in either of the ways above. With
`lastNumberOfFixes=1000` and `numberOfPoints=10`, the answer covers the same span as the
1000 would have, in 10 positions.

- **`numberOfPoints`** keeps the first and the last position plus every Nth in between. It
  engages only when more positions were selected than the number asked for, so a period
  holding fewer comes back whole, and no threshold on the length of a period is needed. The
  step is a whole number, so the count overshoots what was asked for: mildly when the
  selection is many times larger (2002 points for 2000 over a year), substantially when it
  is between one and two times larger (1440 points for 1001 over two days). The spacing is
  even in positions, not in time.
- **`taperAccuracy`** leaves the most recent third of the requested points unstrided and
  strides the remainder over the rest of the period. That third is a count, so how much
  time it covers follows the reporting rate: with `numberOfPoints=1001` it is 332
  positions, which is 5.5 hours for a platform reporting every minute.

## What odss2dash sends

Every request carries a `startDate`, derived when the caller did not give one,
and `lastNumberOfFixes` is never sent at all, since it would void the dates.
A limit that goes with a date is applied by odss2dash to the response.

| the request | to ODSS | why |
|---|---|---|
| a period (`startDate` given) | `startDate`, `endDate`, `numberOfPoints`, `taperAccuracy=true` | one response covering the period, its recent part intact |
| the latest positions | `startDate` derived from `defaultPositionsWindow`, `endDate` if given | without a lower bound the answer is the complete track, by the slowest query |
| the dispatcher's poll | `startDate` from the position it last reported, floored at `dispatchCatchUpWindow` | only what it has not dispatched yet, and never a replay of an outage |

Measured through odss2dash for `D_PACKARD`, which reports every minute:

| request | response |
|---|---|
| latest positions, limit 20 | 20 positions, 0.3s |
| 12 hour period, limit 1001 | 719 positions, none strided, 0.2s |
| 2 day period, limit 1001 | 1182 positions: newest 332 every minute, then every 3 minutes, 0.1s |
| 1 year period, limit 1001 | 1001 positions: newest 332 every minute, then every 12 hours, 4.6s |

## Other observations

- **Responses are not compressed.** `Accept-Encoding: gzip` returns the same byte count, so
  the size of a response matters as much as the time to produce it.
- **Status codes do not distinguish a missing platform from a failure.** An unknown platform
  on `/tracks` answers `500` with a JSON body reporting a failure, and a request missing
  `platformID` also answers `500`, with a body that reports success. odss2dash therefore
  does not ask ODSS whether a platform exists: it consults its own platform cache.
- **The unbounded request is the slow one.** Asking for the latest 20 positions with no
  dates takes 33s for `brizo_ac` and `DAllanB`, and does not answer within 60s for
  `makai_ac` and `wgSpare`, while a 7 day window for those same platforms answers in 0.08
  to 0.16s. It is not simply a matter of how many positions a platform has: `W_FLYER`, with
  3.4M of them, answers the unbounded request in 0.1s. Always sending a lower bound is what
  keeps odss2dash clear of this.
- **Reporting fewer points does not make a request cheaper upstream.** A one year request
  takes about 3s to first byte whether it answers with 23.6MB or, with `numberOfPoints`,
  96KB. The thinning saves the transfer and everything downstream of it, not the work ODSS
  does. Only the lower bound odss2dash derives keeps that work bounded.

## Worth raising with the ODSS maintainers

`lastNumberOfFixes` reads naturally as a limit: *the last N positions up to `endDate`*. It
is instead an alternative to the dates, and voids them silently. If it were applied as a
limit on the positions the dates select, a period with a limit would become expressible in
one request, and no existing caller would see a difference: a request carrying only
`lastNumberOfFixes` would be answered exactly as it is today, and the only combination that
would change meaning is one that currently answers with positions outside the range it was
given.

With that in place, odss2dash could pass the caller's limit and the period together, and
`defaultPositionsWindow` would no longer be needed.
