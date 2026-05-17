# AWS examples

Deploy targets on AWS (Lambda, API Gateway, etc.).

## Examples

| Directory | Description |
|-----------|-------------|
| [`order-api-hello-lambda`](order-api-hello-lambda/README.md) | **kernl** mini REST demo (`kn/order_api.knl`): fake orders/customers JSON + CDK HTTP API on Lambda (**`provided.al2023`** zip). |

## Shared prerequisites

- [AWS CLI](https://docs.aws.amazon.com/cli/latest/userguide/getting-started-install.html) configured (`aws configure` or SSO).
- Per-example tools (Rust, Node/npm for CDK, cross-compile helpers): see each example’s README.
