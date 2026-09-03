# SyncHalo Privacy Policy

Last updated: 2026-09-03

SyncHalo is a local-first, open-source application. The SyncHalo project does
not operate an application server, analytics service, advertising service, or
cloud clipboard/file relay, and it does not collect or sell personal data.

Clipboard text, selected files, device identities, pairing material, settings,
and transfer history stay on the user's devices. Content is transferred only
between devices that the user pairs on the local network. Clipboard history is
encrypted locally; private keys and file bytes are not exposed to the WebView.

Production builds check the public SyncHalo GitHub Release endpoint for signed
updates. This request necessarily exposes normal network metadata, such as the
device IP address and HTTP headers, to GitHub under the
[GitHub Privacy Statement](https://docs.github.com/en/site-policy/privacy-policies/github-general-privacy-statement).
Automatic update checks can be disabled in SyncHalo settings. Ubuntu users who
add the SyncHalo APT repository make update requests to GitHub Pages.

Operating systems and desktop environments may independently record crash or
usage information according to their own settings. SyncHalo does not upload
those reports to the project.

Privacy questions and reports can be opened at
<https://github.com/macji/synchalo/issues> without including clipboard content,
private keys, pairing codes, or sensitive file paths.
