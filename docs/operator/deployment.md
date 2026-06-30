# Deployment

## Docker image

The published image runs as the `envoy` user (uid `101` in the upstream `envoyproxy/envoy` base image).

To override the uid at build time (e.g. when rebasing on a base image where the `envoy` user has a different uid):

```bash
docker build --build-arg ENVOY_UID=1001 .
```

The `ENVOY_UID` build argument is provided as an override surface; the `chown` step uses the named `envoy` user directly.
