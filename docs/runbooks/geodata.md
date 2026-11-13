# Geodata runbook

The world's terrain (v0.2a onwards) is a **datapack**: GeoPack archives (`.gpk`) built from public
elevation data by `tools/geo-pipeline` and served from a Cloudflare R2 bucket (ADR-50, ADR-55). The
app itself stays on GitHub Pages. Geodata is never committed to this repository.

## The owner's R2 setup (once, about 20 minutes)

1. Sign in at `dash.cloudflare.com` (create a free account if needed). R2 may ask for a payment method
   even inside the free tier [verify].
2. **R2 → Create bucket**: name `flying-geo`, location Automatic.
3. **Bucket → Settings → Public access → R2.dev subdomain → Allow.** Copy the URL
   (`https://pub-<id>.r2.dev`). It is rate-limited and meant for development (owner decision D3: fine
   for v0.2; a custom domain before the public beta).
4. **Bucket → Settings → CORS policy → Add**, paste:
   ```json
   [ { "AllowedOrigins": ["https://bl4202.github.io", "http://localhost:4173", "http://localhost:4174"],
       "AllowedMethods": ["GET", "HEAD"], "AllowedHeaders": ["Range"],
       "ExposeHeaders": ["Content-Range", "Content-Length", "ETag"], "MaxAgeSeconds": 86400 } ]
   ```
5. **R2 → Manage R2 API tokens → Create API token**: permissions *Object Read & Write*, *Apply to
   specific buckets only* → `flying-geo`. Copy the **Access Key ID**, the **Secret Access Key** and your
   **Account ID** (in the S3 endpoint `https://<account-id>.r2.cloudflarestorage.com`).
6. **GitHub → Bl4202/Flying → Settings → Secrets and variables → Actions**: secrets `R2_ACCOUNT_ID`,
   `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`; variables `R2_BUCKET` = `flying-geo` and
   `GEO_PUBLIC_BASE` = `https://pub-<id>.r2.dev/` (with the trailing slash).
7. Tell the agent the r2.dev URL. **Never paste the keys into a chat or an issue.**
8. Cost (prices of 2026-11-12, ADR-50 [verify]): about 19 GB stored (measured compression, ADR-54;
   the plan estimated 11 GB) → about $0.14 a month above the 10 GB free tier; reads are free up to
   10 million a month; no egress fee; no hard spending cap.

## How a datapack is published

A datapack is published from `main` by the manual workflow `.github/workflows/geo-publish.yml`
(owner decision D2, ADR-51). Its id is `geo-YYYY.MM`, the month of the run in UTC.

1. Check the repository secrets and variables: `gh secret list` shows `R2_ACCOUNT_ID`,
   `R2_ACCESS_KEY_ID` and `R2_SECRET_ACCESS_KEY`; `gh variable list` shows `R2_BUCKET` and
   `GEO_PUBLIC_BASE` (the r2.dev URL with a trailing `/`).
2. Dry run, the region cells only, nothing uploaded:
   `gh workflow run geo-publish.yml -f datapack_id=geo-YYYY.MM -f cells=regions -f dry_run=true`,
   then `gh run watch`. Fix and repeat until it is green.
3. The real run, every cell with land:
   `gh workflow run geo-publish.yml -f datapack_id=geo-YYYY.MM -f cells=all -f dry_run=false`.
   The `cells` jobs upload each cell archive as it is built; the `base` job builds `base.gpk`, the QA
   report and the licence audit, uploads them, and uploads `datapack.json` **last**, so the new id
   is never named before every file it lists exists.
4. Check by hand: `curl -sS <GEO_PUBLIC_BASE>datapack.json | head -c 400`, and a range read with an
   `Origin: https://bl4202.github.io` header returns `206` with `Access-Control-Allow-Origin` and
   `Content-Range`.
5. Copy the run's `qa.json` and `licences.json` into `docs/exit/<version>/datapack/` with a
   `README.md` (the run link, the cell count, bytes per level, the R2 storage total).

A failed run can be dispatched again with the same id: objects are overwritten, and none of them is
used before `datapack.json` names the id.

## How to build the fixture locally

The synthetic fixture (two islands in a sea, ADR-51 "Section 9.5") is what CI, the determinism suite
and the e2e specs use. It needs no network:

```sh
cargo xtask geo-fixture          # writes target/geo-fixture/
cargo xtask geo-fixture --repro  # builds it twice and compares every file (GP1)
node packages/tools/src/geo-server.ts target/geo-fixture --port 4175   # serve it to the app
```

Then open the app with `?world=alps&geo=http://localhost:4175/`.

## Where data comes from

Copernicus DEM, the 2021 release, on public AWS buckets (HTTPS, no account):

- GLO-90 (3 arc-seconds, about 90 m): `https://copernicus-dem-90m.s3.amazonaws.com/<name>/<name>.tif`
  with `<name>` = `Copernicus_DSM_COG_30_N46_00_E007_00_DEM` (for the 1° tile N46 E007).
- GLO-30 (1 arc-second, about 30 m): the same pattern with `copernicus-dem-30m` and `COG_10`.
- Tile lists: `https://copernicus-dem-90m.s3.amazonaws.com/tileList.txt` and
  `https://copernicus-dem-30m.s3.amazonaws.com/tileList.txt` (one name per line).
- **Ocean areas have no tile; their height is 0**, and the sea inside a tile is exactly 0.0.
- The files are tiled GeoTIFF, float32, zlib, predictor 3, PixelIsPoint, EPSG 4326. Pixel (0, 0)'s
  centre is at the tile's west longitude and its north latitude + 1°.

Downloads go to `target/geo-cache/` (git-ignored). The licence and the attribution the game shows are
in `content/world/sources/copernicus_dem.json`; how they were checked is in `content/world/README.md`.

## Rolling back a datapack

Old datapacks stay in the bucket under their own id. To go back, set `latest` in `datapack.json` to
the previous id and upload it again:

```sh
node packages/tools/src/r2-put.ts --get datapack.json > datapack.json      # needs the R2_* variables
# edit "latest"
node packages/tools/src/r2-put.ts --file datapack.json --key datapack.json \
  --content-type application/json --cache-control "public, max-age=60"
```

Clients pick the change up within a minute (`max-age=60`).
