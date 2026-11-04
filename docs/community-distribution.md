# Umbra community distribution

The client is free for noncommercial hobby use and does not require a purchase or activation key.
Distribute the repository's `LICENSE` and third-party notices with each release. This project is
derived from Cinnabar under PolyForm Noncommercial 1.0.0, so do not sell access, bundle it with a
paid product, or use it for a commercial service without separate permission from the relevant
rights holders. A copy of the terms is at <https://polyformproject.org/licenses/noncommercial/1.0.0>.

## Publish checklist

1. Review and push the client source to the public `qHideo/umbra-client` repository. Do not push
   build outputs, `.local` credentials, signing seeds, tokens, or local rank registries.
2. GitHub Pages is configured to use **GitHub Actions**. After the source is pushed, the `Umbra
   website` workflow stages and deploys the landing page on the `main` or `dev` branch; it is
   explicitly skipped on the upstream Cinnabar repository. The page discovers assets attached to
   the latest GitHub release.
3. Change `packaging/release-assets.json` to the Umbra repository slug and rename the release
   assets from Cinnabar to Umbra. Check installer names, desktop metadata, update channels, and
   generated install scripts for upstream repository URLs before creating a tag.
4. Configure the repository's release signing secrets and trusted public update keys. Never reuse
   a private signing seed in a public commit or artifact.
5. Run the Windows, Linux, and macOS package workflows and review each artifact and its notices.
   The package font is Inter under OFL-1.1; its license is included. Cinnangles faces must not be
   added to release payloads unless their redistribution rights are resolved.
6. Publish the artifacts and checksum file on the Umbra repository's release page. Point the
   README download links there only after that release exists. Attach the ready
   `Umbra-Client-Windows-x64.zip` portable build to the first release and include its SHA-256 in
   the release notes; the landing page reads this filename from `packaging/release-assets.json`.

## Player ranks

Owner ranks are cosmetic and server-signed. Source-run clients can use the loopback rank service,
but it is reachable only on the same computer. For ranks to work for downloaded clients, deploy
the rank lookup API behind HTTPS and configure the client with that public endpoint and its
matching public verification key. If no public endpoint exists, lookups safely return no owner
rank; gameplay remains available.

## Before inviting downloads

The existing `origin` remote still points to `cinnabar-mc/cinnabar`; do not push Umbra changes there.
Use the `umbra` remote for `qHideo/umbra-client`. Pages is configured, but the landing page will not
be live until the reviewed source and its workflow have been pushed and the first deployment
succeeds.
