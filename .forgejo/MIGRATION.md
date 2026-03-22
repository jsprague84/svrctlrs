# Migrating SvrCtlRS to Forgejo

## 1. Create repo on Forgejo

Go to `https://git.js-node.cc/jsprague/` and create a new empty repo `svrctlrs`.

## 2. Push the repo

```bash
cd ~/Development/svrctlrs
git remote add forgejo git@git.js-node.cc:jsprague/svrctlrs.git
git push forgejo --all
git push forgejo --tags
```

## 3. Build and push CI images

Build images locally and push to the Forgejo registry.
Each image layers on the previous one, so build in order.

```bash
# Login to Forgejo registry
docker login git.js-node.cc

# 1. Base CI image (if not already built from rsthere)
# docker buildx build -t git.js-node.cc/jsprague/rust-ci:latest -f path/to/rsthere/.forgejo/Dockerfile.ci .
# docker push git.js-node.cc/jsprague/rust-ci:latest

# 2. SvrCtlRS CI image (Rust + Node.js)
docker buildx build -t git.js-node.cc/jsprague/svrctlrs-ci:latest -f .forgejo/Dockerfile.ci .
docker push git.js-node.cc/jsprague/svrctlrs-ci:latest

# 3. Tauri Desktop CI image (adds WebKitGTK deps)
docker buildx build -t git.js-node.cc/jsprague/tauri-desktop-ci:latest -f .forgejo/Dockerfile.tauri-desktop .
docker push git.js-node.cc/jsprague/tauri-desktop-ci:latest

# 4. Tauri Android CI image (adds Android SDK/NDK — takes ~10 min)
docker buildx build -t git.js-node.cc/jsprague/tauri-android-ci:latest -f .forgejo/Dockerfile.tauri-android .
docker push git.js-node.cc/jsprague/tauri-android-ci:latest
```

## 4. Create CI cache directories

Run once inside the DinD container:

```bash
docker exec forgejo-dind mkdir -p \
  /ci-cache/sccache \
  /ci-cache/cargo-registry \
  /ci-cache/npm-cache \
  /ci-cache/gradle
```

Verify the `ci-cache` volume is mounted in your DinD service
(see `docker-compose.ci-cache.yaml` from rsthere — same setup).

## 5. Configure Forgejo secrets

Go to repo Settings → Actions → Secrets and add:

| Secret | Purpose |
|--------|---------|
| `RELEASE_TOKEN` | Forgejo API token for creating releases and uploading assets |
| `REGISTRY_TOKEN` | Forgejo registry token for pushing Docker images |
| `ANDROID_KEYSTORE_BASE64` | (Optional) Base64-encoded Android signing keystore |
| `ANDROID_KEYSTORE_PASSWORD` | (Optional) Keystore password |
| `ANDROID_KEY_PASSWORD` | (Optional) Key password |

Generate tokens at: `https://git.js-node.cc/user/settings/applications`

For the Android keystore (optional, for signed release builds):
```bash
base64 -w0 your-release.keystore | xclip -selection clipboard
```

## 6. Verify workflows

Push a test commit or tag to trigger the workflows:

```bash
# Test CI pipeline
git push forgejo develop

# Test full release (Docker + Desktop + Android)
git tag v0.1.0
git push forgejo v0.1.0
```

## Workflow summary

| Workflow | Trigger | Outputs |
|----------|---------|---------|
| `ci.yaml` | push/PR to main, develop | lint, test, frontend check |
| `docker.yaml` | push to main, version tags | Docker image → `git.js-node.cc/jsprague/svrctlrs` |
| `desktop.yaml` | version tags | .deb, .rpm, .AppImage → release assets |
| `android.yaml` | push to main (debug), version tags (release) | APK → release assets |

## Optional: Set Forgejo as primary remote

```bash
git remote set-url origin git@git.js-node.cc:jsprague/svrctlrs.git
git remote add github git@github.com:jsprague84/svrctlrs.git
```

Or mirror pushes to both:
```bash
git remote set-url --add --push origin git@git.js-node.cc:jsprague/svrctlrs.git
git remote set-url --add --push origin git@github.com:jsprague84/svrctlrs.git
```
