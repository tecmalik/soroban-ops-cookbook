#!/usr/bin/env node
/**
 * Interactive generator for Anchor Platform docker-compose.yml + .env.
 *
 * This asks a handful of questions and fills in the templates in
 * ./templates/ so a first-time integrator gets a runnable stack instead of
 * having to cross-reference the Anchor Platform docs for every env var.
 *
 * NOTE: The exact env var names in templates/*.template are based on the
 * Anchor Platform docs at the time this was written. Anchor Platform's
 * config schema changes between versions — verify against
 * https://developers.stellar.org/docs/platforms/anchor-platform before
 * relying on generated output in production. This is exactly the kind of
 * drift a `good-first-issue` here could keep in sync.
 */
const fs = require("fs");
const path = require("path");
const prompts = require("prompts");

async function main() {
  const answers = await prompts([
    {
      type: "text",
      name: "homeDomain",
      message: "Home domain (e.g. anchor.example.com)",
    },
    {
      type: "select",
      name: "network",
      message: "Stellar network",
      choices: [
        { title: "Testnet", value: "TESTNET" },
        { title: "Pubnet (mainnet)", value: "PUBNET" },
      ],
    },
    {
      type: "multiselect",
      name: "seps",
      message: "Which SEPs do you need enabled?",
      choices: [
        { title: "SEP-10 (Web Auth)", value: "sep10", selected: true },
        { title: "SEP-12 (KYC API)", value: "sep12" },
        { title: "SEP-24 (Hosted deposit/withdrawal)", value: "sep24" },
        { title: "SEP-31 (Cross-border payments)", value: "sep31" },
        { title: "SEP-38 (Quotes)", value: "sep38" },
      ],
    },
    {
      type: "text",
      name: "assetCode",
      message: "Primary asset code (e.g. USDC)",
      initial: "USDC",
    },
    {
      type: "text",
      name: "assetIssuer",
      message: "Asset issuer public key",
    },
    {
      type: "text",
      name: "kycProvider",
      message: "KYC provider name (informational only, for now)",
      initial: "none",
    },
  ]);

  // Validate required fields.
  if (!answers.homeDomain) {
    console.error("Error: home domain is required.");
    process.exit(1);
  }
  if (!answers.assetCode) {
    console.error("Error: asset code is required.");
    process.exit(1);
  }
  if (!answers.assetIssuer) {
    console.error("Error: asset issuer public key is required.");
    process.exit(1);
  }
  if (
    !answers.assetIssuer.startsWith("G") ||
    answers.assetIssuer.length !== 56
  ) {
    console.error(
      "Warning: asset issuer doesn't look like a Stellar public key " +
        "(expected G... + 56 chars). Continuing anyway — double-check before deploying."
    );
  }
  if (!answers.seps || answers.seps.length === 0) {
    console.error("Error: at least one SEP must be selected.");
    process.exit(1);
  }

  const enabledSeps = answers.seps.join(",");
  const assetsJson = JSON.stringify([
    { code: answers.assetCode, issuer: answers.assetIssuer },
  ]);
  const dbPassword = Math.random().toString(36).slice(2, 14);
  const sep10Seed = "REPLACE_WITH_A_REAL_SEP10_SIGNING_SEED";

  const replacements = {
    "{{HOME_DOMAIN}}": answers.homeDomain,
    "{{NETWORK}}": answers.network,
    "{{ENABLED_SEPS}}": enabledSeps,
    "{{ASSETS_JSON}}": assetsJson,
    "{{SEP10_SIGNING_SEED}}": sep10Seed,
    "{{DB_PASSWORD}}": dbPassword,
    "{{KYC_PROVIDER}}": answers.kycProvider,
  };

  const outDir = path.join(__dirname, "output");
  fs.mkdirSync(outDir, { recursive: true });

  for (const [templateFile, outFile] of [
    ["docker-compose.template.yml", "docker-compose.yml"],
    ["env.template", ".env"],
  ]) {
    let content = fs.readFileSync(
      path.join(__dirname, "templates", templateFile),
      "utf8"
    );
    for (const [key, value] of Object.entries(replacements)) {
      content = content.split(key).join(value);
    }
    fs.writeFileSync(path.join(outDir, outFile), content);
  }

  console.log(`\nGenerated config in ${outDir}/`);
  console.log(
    "IMPORTANT: replace SEP10_SIGNING_SEED with a real secret key before running.\n" +
      "Verify env var names against the current Anchor Platform docs before deploying."
  );
}

main();
