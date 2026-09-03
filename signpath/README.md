# SignPath Foundation setup

Formal SyncHalo releases use SignPath Foundation to Authenticode-sign both
Windows installers. The certificate and private key remain in SignPath's HSM;
the repository stores only an API token that can submit policy-controlled
signing requests.

This is a one-time account setup and cannot be performed by the release
workflow because it includes identity review and acceptance of SignPath's OSS
terms.

## Apply and connect the repository

1. Apply at <https://signpath.org/apply> using
   <https://github.com/macji/synchalo> as the project URL.
2. Provide the repository's `PRIVACY.md`, `SECURITY.md`, and
   `CODE_SIGNING_POLICY.md` links during review.
3. Require two-factor authentication for every GitHub and SignPath maintainer.
4. Install the <https://github.com/apps/signpath> GitHub App for this repository.
5. In SignPath, add the predefined `GitHub.com` trusted build system to the
   organization and link it to the project.

## Use the workflow's exact names

- Project slug: `synchalo`
- Signing policy slug: `release-signing`
- Artifact configuration slug: `windows-installers`
- Artifact configuration: paste `signpath/artifact-configuration.xml`

The release policy must require manual approval. Create an API token for a
submitter that can use this project and policy, then configure the repository:

```bash
gh secret set SIGNPATH_API_TOKEN --repo macji/synchalo
gh secret set SIGNPATH_ORGANIZATION_ID --repo macji/synchalo
```

The workflow uploads the MSI and NSIS installers as a short-lived GitHub
Actions artifact, submits that artifact to SignPath, waits for approval, and
verifies that both returned files have a valid `SignPath Foundation`
Authenticode publisher. Because Authenticode changes the installer bytes, the
workflow regenerates the Tauri updater signatures only after SignPath returns.

Manual `windows` workflow runs remain unsigned Actions artifacts. An `all`
run, including every pushed `v*` tag, fails closed if SignPath is unavailable
or returns an unexpected publisher.
