# Cloud deployment examples

Minimal reference deployments for HTTP APIs and related patterns. Each provider lives under its own directory so we can add **GCP**, **Azure**, and others without reshuffling paths.

## Layout

| Path | Description |
|------|-------------|
| [`aws/`](aws/README.md) | Amazon Web Services (Lambda, API Gateway, etc.) |
| [`gcp/`](gcp/README.md) | Google Cloud (placeholders for future examples) |
| [`azure/`](azure/README.md) | Microsoft Azure (placeholders for future examples) |

## Contributing

When adding a new example:

1. Put it under the right provider folder (e.g. `aws/<example-name>/`).
2. Include a **README** with prerequisites, deploy commands, test `curl`, and teardown.
3. Prefer infrastructure-as-code (SAM, Terraform, etc.) and pin **runtimes** explicitly.
