# Anchor Configs

A tiny CLI that generates a working `docker-compose.yml` + `.env` for SDF's
[Anchor Platform](https://developers.stellar.org/docs/platforms/anchor-platform)
from a short interactive Q&A, instead of hand-assembling config from the
Anchor Platform docs.

This does **not** replace the Anchor Platform — it just removes the
"which of these 30 env vars do I actually need for SEP-24 deposit-only"
friction that trips up most first integrations.

## Usage

```bash
cd anchor-configs
npm install
node generate-config.js
```

You'll be asked a handful of questions (which SEPs, which assets, testnet or
mainnet, KYC provider) and get a ready-to-run `docker-compose.yml` + `.env`
in an `output/` folder.

## Roadmap (see issues)

- [ ] SEP-38 (quotes) config support
- [ ] Preset for common payment-rail providers (Flutterwave, MoneyGram-style)
- [ ] Validation against Anchor Platform's actual schema before writing files
