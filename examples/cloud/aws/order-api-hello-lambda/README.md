# Ordering API demo — kernl on Lambda (CDK, zip / provided.al2023)

Business logic lives in **`kn/order_api.knl`**: it reads a small **stdin protocol**, runs **list/string helpers** (`split`, `head`, `tail`, `snoc`, `filter_not_id`, recursive JSON assembly), and **`print`s a multi-line stdout protocol**. **`lambda-bootstrap`** keeps an **in-memory `Vec<Order>`** (warm-container only — resets on cold start) and refreshes it from kernl when responses include **`__STATE__`**.

AWS Lambda still runs **`kernlc kn/order_api.knl --invoke-stdin --run`** (`compiler` flag **`--invoke-stdin`** feeds **all of stdin** into `main`’s single `str` parameter).

This example follows the **`demo-stacks`** style: **AWS CDK** deploys an HTTP API + Lambda zip built into **`dist/`**. **`cdk deploy` does not require Docker** on your machine.

## Stdin protocol (bootstrap → kernl)

Three logical parts, newline-separated:

1. **Operation key** — `health`, `customers.list`, `orders.list`, `orders.get`, `orders.add`, `orders.delete`, `orders.update`, or `unknown:METHOD:path`.
2. **Payload line** — route-specific (`orders.get` / `orders.delete`: order id only; `orders.add` / `orders.update`: full TSV row `id\tcustomerId\ttotalCents\tstatus`; others: empty).
3. **Zero or more order rows** — each `id\tcustomerId\ttotalCents\tstatus` (TSV).

## Stdout protocol (kernl → bootstrap)

1. **HTTP status** — `200` or `404` (plain text).
2. **JSON body** — the **inner** JSON object or array (not the API Gateway envelope).
3. Third line:
   - **`__KEEP_STATE__`** — bootstrap leaves its store unchanged.
   - **`__STATE__`** — following lines **replace** the store (zero or more TSV rows).

## HTTP routes (after deploy)

| Method | Path | Operation key |
|--------|------|----------------|
| GET | `/health` | `health` |
| GET | `/customers` | `customers.list` |
| GET | `/orders` | `orders.list` |
| GET | `/orders/{id}` | `orders.get` |
| POST | `/orders` | `orders.add` |
| PATCH | `/orders/{id}` | `orders.update` |
| DELETE | `/orders/{id}` | `orders.delete` |

Unknown paths use keys `unknown:METHOD:path` and fall through to the kn **404** JSON.

Stack outputs **`OrdersListUrl`**, **`OrderDetailExampleUrl`**, **`CustomersListUrl`**, **`HealthUrl`**, plus **`HttpApiBaseUrl`**.

## Prerequisites

- **Rust** toolchain (`cargo`, `rustup`) — needs **`kernlc`** from this repo (includes **`--invoke-stdin`** and stdlib helpers used by the demo).
- **Zig** + **`cargo-zigbuild`** — cross-compile Linux `x86_64` from macOS **without Docker**:
  - Zig: [ziglang.org/download](https://ziglang.org/download/)
  - `cargo install cargo-zigbuild`
- **Node.js** + **npm** (for CDK).
- **AWS CLI** credentials (`aws sts get-caller-identity` works).

Optional: run **`build-lambda.sh` on Linux** with `cargo build --release --target x86_64-unknown-linux-gnu` instead of zigbuild (edit the script).

## Layout

```
order-api-hello-lambda/
├── README.md
├── build-lambda.sh          # produces ./dist (kernlc + bootstrap + kn/)
├── lambda-bootstrap/        # Rust Lambda entry → kernlc + store merge
├── cdk/
├── kn/
│   └── order_api.knl        # CRUD helpers + routing
└── dist/                    # generated — do not commit
```

## kernl notes

- **`do`** is a **single** expression; multi-step bodies use **`if true … end`** blocks.
- **`|`-pipes are right-associative**: avoid long chains like `a | split … | head`; prefer **`let parts = a | split …`** then **`head parts`** (same issue with unary builtins such as **`len`** / **`head`** after a pipe — use **`len xs`** or **`head xs`**).
- Nested **`concat`** calls are **not** parsed as nested calls unless you **`let`** intermediates (only two atoms per **`concat`**).
- **`match`** arm bodies still end at newline — **`main`** keeps **one call per arm**.

## Build the Lambda bundle

From **`order-api-hello-lambda/`**:

```bash
chmod +x build-lambda.sh
./build-lambda.sh
```

This writes **`dist/`** with `bootstrap`, `kernlc`, and **`kn/`** (including `order_api.knl`).

## Deploy (CDK)

Run **`cdk` from `order-api-hello-lambda/cdk/`** (the app resolves **`../dist`** from that directory).

```bash
cd cdk
npm install
npm run build
npx cdk bootstrap   # once per account/region
npx cdk deploy --require-approval never
```

### Destroy

```bash
cd cdk
npx cdk destroy --force
```

## Try it

```bash
BASE="$(aws cloudformation describe-stacks \
  --stack-name KernlOrderHelloStack \
  --query 'Stacks[0].Outputs[?OutputKey==`HttpApiBaseUrl`].OutputValue' \
  --output text)"

curl -sS "${BASE}health"
curl -sS "${BASE}orders"
curl -sS "${BASE}orders/ord-1001"
curl -sS -X POST "${BASE}orders" \
  -H 'Content-Type: application/json' \
  -d '{"customerId":"cust-demo","totalCents":999,"status":"open"}'
curl -sS -X PATCH "${BASE}orders/ord-1002" \
  -H 'Content-Type: application/json' \
  -d '{"status":"shipped"}'
curl -sS -X DELETE "${BASE}orders/ord-1002"
curl -sS "${BASE}customers"
```

## Local smoke test (optional)

Multiline stdin (matches Lambda bootstrap):

```bash
printf 'orders.list\n\nord-1001\tcust-42\t1299\tpaid\n' | cargo run --manifest-path ../../../../compiler/Cargo.toml -- \
  kn/order_api.knl --invoke-stdin --run
```

Paths assume you run from **`order-api-hello-lambda/`**; adjust **`--manifest-path`** to your **`kernl/compiler/Cargo.toml`** if your tree differs.

Without **`--invoke-stdin`**, stdin is not read; `main` still receives an empty string (404 / unsupported).

## Why Rust `bootstrap` instead of shell + curl?

**`provided.al2023`** zip bundles don’t ship **`curl`**. **`lambda_runtime`** handles the Runtime API; **`kernlc`** stays the interpreter for **`.knl`**.

Fully compiling kernl to one ELF that implements the Runtime API is a possible later step.
