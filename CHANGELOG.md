2025-02

2.3.4

- use ureq (a pure Rust http client)
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
