# Umbra license service (not used by free community builds)

This repository retains optional server-verified license tooling. The local `umbra-license` tool creates
one-time keys, stores only peppered key hashes, enforces device limits and issues Ed25519-signed,
device-bound leases. The client stores the reusable key in the operating system credential vault
and accepts a signed lease offline for up to seven days. Online renewals check expiry, revocation
and device limits; an already issued offline lease may continue until its signed expiry.

The repository does not provide a hosted endpoint or payment integration. Put the API behind an
HTTPS reverse proxy and configure client builds or launch environments with
`UMBRA_LICENSE_API_URL=https://your-license-host/`. The service itself only binds to loopback.
Plain HTTP is accepted by the client only for localhost development.

## Local operator setup

From the repository root, initialize one signing key and pepper:

```powershell
cargo run -p license-service -- init
```

This writes raw secret bytes to ignored `.local/license/signing.seed` and `.local/license/pepper`
and updates the tracked public verification key in `crates/license/keys/public.key`. Back up the
secret directory securely. Do not regenerate or share it: changing it invalidates leases and
requires rebuilding every client with the matching public key.

Create a purchase/access key. The displayed key is shown once, so deliver and store it securely:

```powershell
cargo run -p license-service -- create --label "Order 1234" --max-devices 2
```

Optional `--expires-at <unix-seconds>` sets the access expiry. The service has no payment
processor; an operator should issue keys only after confirming payment.

Start the local API:

```powershell
cargo run -p license-service -- serve
```

For local client activation, set `UMBRA_LICENSE_API_URL=http://127.0.0.1:8788/`. For remote
users, reverse proxy HTTPS traffic to `127.0.0.1:8788`, then configure the client with the public
HTTPS URL. Activate a key from the client with `--activate-license`; the prompt hides the key.

Manage licenses with `list` and `revoke <license-id>`. Revocation is enforced on the next online
renewal. A disconnected device can use its existing signed lease until its seven-day maximum
offline expiry. The client gate raises the bar against casual sharing and patching, but software
running on a device controlled by its owner cannot be made impossible to modify. Keep server
secrets private, rate-limit the public reverse proxy, and distribute only signed release builds.

## Owner ranks

Owner badges are cosmetic. The client requests ranks for the current player roster and only accepts
short lived Ed25519-signed answers from the same service. The normal Umbra badge is gray; an owner
badge is amber. Rank assignments are stored in the server's ignored `.local/license/ranks.json`
registry and can only be changed by an operator with access to the server host. There is no HTTP
route for granting ranks.

Grant, list, or revoke a rank by stable Xbox XUID or UUID:

```powershell
cargo run -p license-service -- rank grant --identity 2535437636938345 --label qHideo
cargo run -p license-service -- rank grant --identity 2535466372161021 --label ty4r
cargo run -p license-service -- rank list
cargo run -p license-service -- rank revoke --identity 2535466372161021
```

The local source launcher sets `UMBRA_RANK_API_URL` to the loopback service. Remote clients need
the rank lookup API exposed through HTTPS and the same trusted signing key embedded in their
builds. Keep the signing seed private. Ranks affect the displayed badge only; authorization must
never rely on this client-side cosmetic status.
