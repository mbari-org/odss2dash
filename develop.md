On and off I've been putting together various pieces for a reimplementation of the odss2dash system in Rust.
Without being trivial, the odss2dash service is simple enough to further explore the Rust ecosystem for
development of web services.
This repo is intended to capture such reimplementation in a more organized way.
(And may eventually become the new codebase of the production system.)

Overall plan:

- [x] Initial version with configuration handling
- [ ] Direct requests to the Tracking DB
- [ ] Dispatch of position polling and notification to configured TethysDash instances
- [ ] The service with REST API and OpenAPI documentation
- [ ] Testing
- [ ] Conclusion