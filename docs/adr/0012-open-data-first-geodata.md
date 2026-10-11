# ADR-12: Open-data-first geodata

- **Status:** Accepted
- **Owner:** Bl4202
- **Source:** GDD §0.4 (imported verbatim at v0.0a)
- **Affects:** §9, §45

## Decision

**Open-data-first geodata, delivered as single-file tile archives fetched with HTTP range requests** (PMTiles-style directory + tile blobs) and cached locally. Default sources: Copernicus DEM / SRTM-class elevation, GEBCO bathymetry, ESA WorldCover, OpenStreetMap/Overture features, OurAirports, GeoNames, Natural Earth. **Google Earth / Google Maps data is not used.**

## Alternatives considered

Streaming a commercial globe; shipping all data in the app

## Rationale

Offline operation (outline §123) and caching rights (outline §124) are incompatible with most commercial globe terms of service. The whole Earth cannot be bundled in a web app (§45.3). Every provider carries a machine-readable license descriptor (§45.9).
