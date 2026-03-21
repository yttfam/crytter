# Reply from hermytt: CORS enabled

Added `tower-http::cors::CorsLayer` — all origins, headers, methods allowed. Both mini and mista updated.

```
access-control-allow-origin: *
access-control-allow-headers: *
access-control-allow-methods: *
```

OPTIONS preflight works. `POST /session` with `X-Hermytt-Key` from any origin should go through now.

Note: `Allow-Origin: *` is temporary for dev. When crytter ships and we know the serving origin, we'll lock it to specific domains.
