# Suzaku Map · 朱雀

A local-first, continuous-writing input method with word and sentence candidates,
optional language models, and a compact companion panel.

面向 Linux / IBus 的连续输入法：先在草稿中选词、续写和修改，再明确提交正文。

## Download

[0.10.0 Linux Alpha](https://github.com/chiharu-kiryu/Suzaku-map/releases/tag/v0.10.0)
provides `.deb`, binary `.tar.gz`, and matching SHA-256 files.
The supported package baseline is **Ubuntu 24.04 amd64, glibc 2.39, IBus,
X11 or GNOME/XWayland**. This is a prerelease, not latest stable;
other platforms and full native Wayland remain experimental.

After downloading the matching files, check the checksum before installation:

```sh
sha256sum -c suzaku_0.10.0_amd64.deb.sha256
sudo apt install ./suzaku_0.10.0_amd64.deb
```

Installation does not activate the input method or start a personal service.
Finish active composition and fully quit Suzaku before upgrading.
For registration and first use, follow [Getting started / 开始使用](docs/users/getting-started.md).

## Input workflow

- English completion, Chinese Pinyin and basic Japanese Romaji/Kana fallback.
- Word and sentence candidates stay available together, with paging.
- Space and punctuation continue the editable draft; Enter or a candidate click submits it.
- Local authored vocabulary works without a model; optional local/cloud providers use a separate protocol.
- The floating panel, bottom layout, tray and configurable shortcuts share the same input workflow.
- Offline vocabulary packs and a small authoring SDK extend language/topic data independently.

In an ordinary active draft, **1–6** adopt a candidate without submitting;
**Alt+0–9 / numeric keypad** enter literal digits.
**Ctrl+Shift+Space** switches English ↔ Chinese and preserves the draft.
See [input behavior](docs/ibus-candidates.md) and [shortcuts](docs/shortcuts.md)
for selection, privacy-field and focus boundaries.

## Models and privacy

Model suggestions are opt-in. Discovery looks for already-running local services;
Suzaku does not install or start a model server. Cloud suggestions require explicit consent.
Linux candidate preferences persist locally by default and can be cleared from the tray.

Read [model configuration](docs/model-providers.md), [privacy](docs/privacy.md)
and [known limitations](docs/known-limitations.md) before enabling providers.

## Documentation

- [Documentation index / 文档目录](docs/README.md)
- [User guides / 使用指南](docs/users/README.md) — installation, input, shortcuts, models and settings
- [Developer handbook / 开发手册](docs/development/README.md) — architecture, build/test and vocabulary contracts
- [Functional network / 功能网络](docs/functional-network.md) — current routes and maturity evidence
- [Release notes / 版本说明](docs/releases/README.md) — changes and verified publication receipts
- [History and audits / 历史与审计](docs/history/README.md) — preserved development records, not current instructions

## Build and contribute

```sh
cargo build --locked --all-features --bin panel --bin linux_ime_host --bin linux_ime_probe --bin suzaku_tool
cargo test --locked --all-features -- --test-threads=1
```

See [development setup](DEVELOPMENT.md), [testing](docs/development/testing.md)
and [contribution guidelines](CONTRIBUTING.md).
Keep batch logs out of the guides; use the [documentation maintenance rules](docs/development/documentation.md).

MIT licensed; see [LICENSE](LICENSE). Dependency notices ship with packages.
External model weights have their own licenses. For security reports, see [SECURITY.md](SECURITY.md).
