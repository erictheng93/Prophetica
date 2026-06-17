# LG-8 PWA Evidence

Run Lighthouse against the HTTPS staging deployment and save the JSON report:

```sh
lighthouse https://staging.example.com --preset=desktop --output=json --output-path=.tmp/lg8-pwa-lighthouse.json
rtk node tools/scripts/record-lg8-pwa-lighthouse-evidence.mjs .tmp/lg8-pwa-lighthouse.json
rtk node tools/scripts/check-lg8-pwa-evidence.mjs
```

The recorder requires an HTTPS URL and a 100/100 PWA category score. It writes
`docs/evidence/lg-8-pwa/latest.json`.
