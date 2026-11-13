# content/world

The **world bundle**: the pilot regions, the data-source descriptors and their strings. `cargo xtask
content` bakes it into `content-out/world.json` (`world-bundle/1`) beside the sim pack. It is **never
part of the sim pack**, so it never moves the state hash; a world session's numbers reach the core
through the world configuration block (ABI v6, ADR-52).

| Path | Format | What |
|---|---|---|
| `regions/<id>.json` | `region/1` | A pilot region: its cells, its battlefield and its air spawn (ADR-51 D4, plan section 9.4) |
| `sources/<id>.json` | `geo_source/1` | A data source's licence terms (GDD 45.9); the pipeline's licence audit and the Credits & data screen read it |
| `locales/en.json` | strings | Region names and previews |

The schemas are `content/schemas/region.schema.json` and `geo_source.schema.json`.

## How the Copernicus DEM descriptor was filled (2026-11-13)

Read on 2026-11-13:

- the data page,
  https://dataspace.copernicus.eu/explore-data/data-collections/copernicus-contributing-missions/collections-description/COP-DEM
  ("The GLO-30 and GLO-90 datasets are available worldwide with a free license", and its "User
  Obligations" notices);
- the licence document it links,
  https://dataspace.copernicus.eu/sites/default/files/media/files/2025-06/copernicus_contributing_mission_data_access_v2_cop_dem_licenses.pdf,
  and its earlier issue, the Mission-specific Annex of 31 October 2022 (the same DEM texts);
- the AWS Open Data registry, https://registry.opendata.aws/copernicus-dem/ ("GLO-30 Public and
  GLO-90 are available on a free basis for the general public under the terms and conditions of the
  Licence").

The packs use the two public instances, **COP-DEM-GLO-30-F** and **COP-DEM-GLO-90-F** ("Full, Free &
Open"). Each has its own licence ("Licence for the use of the Copernicus WorldDEM-30" and "-90"):

- **Article 3 (Scope):** "The rights granted under this Licence are worldwide and without limitation
  in time."
- **Article 4 (Right of Use):** "(a) reproduction; (b) distribution; (c) communication to the General
  Public; (d) adaptation, modification and combination with other data and information."
- **Article 5:** "The use rights granted under this licence are free of charge to the User."

Neither licence restricts the purpose of use. The non-commercial clause of the general
Copernicus Contributing Mission licence (3.4.3) does not apply to them, by its own 3.4.5: "These
restrictions do not apply where such datasets are otherwise legally available to the User without
the present restrictions". So:

- `commercial_ok` is true (Articles 3 to 5: no purpose restriction; ADR-50 records it as
  commercial-safe);
- `cache_allowed` is true (4(a), reproduction);
- `redistribution_allowed` is true (4(b) to (d), distribution and adaptation);
- `share_alike` is false (no such clause).

**Article 6, User Obligations,** and what the game does about each:

- **(a) and (b) Notices.** The tiles are resampled and re-tiled, so they are adapted (6(b)), and the
  notice is "produced using Copernicus WorldDEM-30 © DLR e.V. 2010-2014 and © Airbus Defence and
  Space GmbH 2014-2018 provided under COPERNICUS by the European Union and ESA; all rights
  reserved", and the same for WorldDEM-90. Both are in `required_notices`, shown in Settings → Data
  → Credits & data (gate LA3).
- **(c) The liability sentence.** Anyone who distributes the data must add "The organisations in
  charge of the Copernicus programme by law or by delegation do not incur any liability for any use
  of the Copernicus WorldDEM-30" (and -90) to the legal notice covering the distribution. Both
  sentences are in `required_notices` and shown with the credits.
- **(d) No endorsement.** The game must not suggest that ESA, the EU or Airbus endorse it. It uses no
  Copernicus or WorldDEM logos (Article 9 grants none), only the names inside the notices.
- **(e) Subsequent users** must be bound by the same obligations if they are granted redistribution
  rights. The game grants players none.

`terms_checked` is the date of this reading. The audit fails a pack built more than 365 days after
it (GDD 45.9), so the descriptor must be read again within a year.
