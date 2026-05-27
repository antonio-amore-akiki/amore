topic: qdrant sha256 verification slsa attestation security
stable: true
# Qdrant Windows ZIP SHA256 Cross-Verification Protocol

Qdrant does not publish a `sha256sum.txt` for its Windows ZIP releases (unlike
ollama which does). The SHA pinned in `scripts/build-installer-windows.ps1` must
therefore be cross-checked manually against two independent vantage points on
each upgrade. This document defines the protocol.

## When to run

Run this protocol before changing `$QdrantZipSha` in `build-installer-windows.ps1`
for any new qdrant release.

## Step 1 — Local SHA from the release asset

```powershell
$QdrantVersion = "v1.18.1"   # adjust per release
$url = "https://github.com/qdrant/qdrant/releases/download/$QdrantVersion/qdrant-x86_64-pc-windows-msvc.zip"
$out = "$env:TEMP\qdrant-verify.zip"
Invoke-WebRequest -Uri $url -OutFile $out -UseBasicParsing
(Get-FileHash $out -Algorithm SHA256).Hash.ToLower()
Remove-Item $out
```

Record this hash as **Hash-A**.

## Step 2 — SLSA provenance attestation (vantage point 1)

Qdrant ships SLSA L3 attestation bundles via `gh attestation verify`:

```bash
VERSION=v1.18.1
ASSET=qdrant-x86_64-pc-windows-msvc.zip

gh release download $VERSION \
  --repo qdrant/qdrant \
  --pattern "$ASSET" \
  --output /tmp/qdrant-verify.zip

gh attestation verify /tmp/qdrant-verify.zip \
  --repo qdrant/qdrant \
  --format json | jq '.[] | .bundle.dsseEnvelope.payload | @base64d | fromjson | .subject[].digest'
```

The `sha256` field in the SLSA subject must match **Hash-A**. If `gh attestation
verify` fails (no attestation available for this asset), fall through to Step 3
and document "SLSA not available" in the update commit message.

## Step 3 — Git tag signature (vantage point 2)

```bash
VERSION=v1.18.1
git clone --depth 1 --branch $VERSION https://github.com/qdrant/qdrant /tmp/qdrant-src
git -C /tmp/qdrant-src verify-tag $VERSION
```

A GPG-verified tag confirms the release was signed by the qdrant maintainers.
Record the signer fingerprint. If tag signature is absent, fall back to verifying
the GitHub release page HTTPS certificate chain (weaker; document accordingly).

## Step 4 — Record result in commit message

When updating the SHA in `build-installer-windows.ps1`, the commit message must
include one of:

```
QDRANT-SHA: Hash-A cross-checked against SLSA L3 attestation (SHA match) +
            Git tag v1.18.1 signed by <fingerprint>. Protocol: docs/QDRANT-SHA-VERIFICATION.md
```

or (if SLSA unavailable):

```
QDRANT-SHA: Hash-A verified by direct download + Git tag v1.18.1 signed by
            <fingerprint>. SLSA attestation not available for this asset.
            Protocol: docs/QDRANT-SHA-VERIFICATION.md
```

## Current pin (v1.18.1)

| Field | Value |
|---|---|
| Version | v1.18.1 |
| Asset | qdrant-x86_64-pc-windows-msvc.zip |
| SHA256 | fe1eab78c24157b21988b3480ce75709e76ca0168ba644fc5a49017bacfec1c6 |
| SLSA check | Pending — Antonio must run Step 2 with authenticated `gh` CLI |
| Tag sig check | Pending — Antonio must run Step 3 |
| Protocol date | 2026-05-27 |

Run this protocol now with your authenticated `gh` CLI. On verification,
update this table and append the result to `docs/results.tsv`.
