# Metrics

envoy-acme exposes the following Prometheus metrics via Envoy's built-in stats sink.

| Metric | Type | Description |
|--------|------|-------------|
| `envoy_acme_issuance_total{result}` | Counter | Total issuance attempts, labelled by result (`success`, `failure`, `permanent`, `recovery_required`). |
| `envoy_acme_consecutive_failures{domain}` | Gauge | Number of consecutive issuance failures for a domain since the last success. |
| `envoy_acme_next_retry_at_seconds{domain}` | Gauge | Unix timestamp of the earliest permitted next retry (0 when not in backoff). |
| `envoy_acme_cert_not_after_seconds{domain}` | Gauge | Unix timestamp of the current certificate's `notAfter` field. |
| `envoy_acme_account_state{domain}` | Gauge | Current account health: `0` = healthy, `1` = recovery required (operator must delete `account.json` and re-register). |
| `envoy_acme_issuance_duration_seconds` | Histogram | Duration of the last issuance attempt in seconds (ceiling-rounded). |

## Recommended alert

```promql
# Page when an ACME account has been stuck in recovery-required state for more than 5 minutes.
envoy_acme_account_state == 1
```
