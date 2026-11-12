# ADR-50: Entering M2: roadmap 2.0, world-data hosting, data licences, pilot regions and panels in a solo project

- **Status:** Accepted
- **Owner:** Bl4202
- **Date:** 2026-11-12
- **Affects:** GDD 0.5, 1.4, 20, 21, 22, 37, 45.3, 45.9, 52, 57.8, 58, 61.5, 66.3, 66.4, 66.6, 66.7,
  D.1, I.2, I.7; `docs/roadmap.md`, `docs/open-questions.md`, ADR-22

## Context

M1 closed with the tag `v0.1d` (2026-11-11). Before M2's first sub-milestone, v0.2a, is planned,
some things had to be decided:

- **Roadmap 2.0** (`docs/roadmap.md`, 2026-11-10) was Proposed. Its content plan (section 3) and
  its 25 reconciliation rows (section 19, R1 to R25) pick one reading wherever the GDD's chapter
  tags, §61.5, §66.3, §67 and the outline disagree. Each row names the GDD text to amend.
- **OQ-03** (hosting, tile bandwidth) and **OQ-04** (commercial or free) are due before M2 (GDD
  0.5). They shape how v0.2a's GeoPacks are delivered and which datasets they may hold.
- **OQ-02** (team size) changes how every duration in the roadmap is read.
- **The six pilot regions** that get full-detail packs in v0.2a (roadmap section 3.9).
- **The human panels** of later gates (G2, G3, G5, G6, G7, the §67.2 run). v0.1d set the rule for
  G1 (ADR-45, D1). The roadmap proposed OQ-12 to decide it for the rest.

The data volumes are the GDD's (§45.3): a base pack of at most 120 MB; z12 terrain for the whole
Earth of order 100 GB compressed; 40 to 150 MB per 5° region cell. GitHub Pages, today's host
(ADR-22), has a soft limit of 100 GB of traffic a month and about 1 GB per site [verify].

## Decision

The owner decided each point on 2026-11-12.

1. **Roadmap 2.0 is accepted in full:** the content plan and rows R1 to R25. The GDD is amended as
   each row says (GDD 0.2.5). R23 was already in 0.2.4. R8, R9, R11, R14, R16, R21 and R24 name no
   GDD text.
2. **OQ-02 is closed: the project is solo.** The roadmap's durations stay relative sizes (R24).
3. **OQ-03: the app stays on GitHub Pages; the world data goes to a Cloudflare R2 bucket.** The
   base pack, `world.gpk` and the region cells are served from R2 with range requests and CORS. R2
   charges nothing for traffic. It has 10 GB-month of storage free, then $0.015 per GB-month, and
   reads cost $0.36 per million after 10 million free a month (Cloudflare's price list,
   2026-11-12) [verify]. The whole-Earth archive is therefore about $1.50 a month. R2 has no hard
   spending cap.
4. **OQ-04 is kept open, with a rule for the data.** The game stays all rights reserved. Until OQ-04
   is decided, a pack takes only data whose licence allows commercial use (`commercial_ok: true` in
   its descriptor, GDD 45.9), so that both a free and a commercial release stay possible. Every
   default source of Appendix H.1 qualifies. OSM's share-alike applies either way. OQ-04's deadline
   moves to before the public beta (v0.5e).
5. **The six pilot regions** are the ones proposed in roadmap section 3.9:
   - the four terrain-character references of `AC-46.7`: Swiss Alps (Alpine), Nevada Basin and
     Range (Rugged), the US Great Plains (Flat) and Normandy (Rolling);
   - the Norwegian fjords (coast and sea);
   - Tokyo Bay (the dense city of PS-4 and `AC-47.1`).
6. **OQ-12 is added and decided: automated proxies gate, as D1 did for G1.** For each human panel
   (G2, the §67.2 run, G3, the usability score, G5, G6, G7, ET-1 to ET-3), the version's gates
   ADR names the automated proxy that decides the gate. The owner's own runs are recorded and do
   not block. A real panel stays possible; its result would be recorded beside the proxy's. The
   GDD's panel thresholds stay as the target for any panel that does run.

## Alternatives considered

- *Hosting:*
  - GitHub Pages only, which fits the base pack and the six regions but not worldwide streaming
    (`AC-3.1`, row R8);
  - deciding the host later, by v0.2d.

  The owner chose R2 so that worldwide streaming can be built and tested from v0.2a.
- *OQ-04:*
  - deciding free now, which would allow non-commercial-only data and close the commercial path;
  - deciding commercial now.

  Keeping both open costs nothing at M2, because the default sources all allow commercial use.
- *G2:* the roadmap recommended a small real panel for G2, the go/no-go on the core concept (risk
  R-05). The owner chose proxies for every panel. Nothing automated measures "fun", so for G2 the
  owner's own sorties are the only signal of it.
- *Accepting only v0.2's rows (R4, R5, R8, R15):* it would leave the GDD contradicting itself for
  later versions.

## Consequences

- **GDD 0.2.5** amends:
  - the chapter tags of §20, §21, §22, §37, §52 and §58;
  - §1.4, §57.8 and §61.5 (which also dates the first damage, missile and pilot implementations to
    v0.1d);
  - §66.3 (M2 to M5), §66.4 (G7, and the panels of OQ-12), §66.6 and §66.7;
  - D.1.

  It also adds the host to §45.3 and the licence rule to §45.9. Its §0.5 records OQ-02, OQ-03 and
  OQ-04, and adds OQ-11 and OQ-12.
- **v0.2a's plan assumes R2.** Before v0.2a needs the bucket, the owner sets up a Cloudflare
  account and an R2 bucket. A custom domain is recommended, because the `r2.dev` address is
  rate-limited and meant for development [verify]. The bucket needs a CORS rule:
  - it allows `GET` and `HEAD` from the staging origin;
  - it allows the `Range` request header;
  - it exposes `Content-Range`, `Content-Length` and `ETag`.

  The staging page is isolated with COEP `require-corp` (ADR-22), so the packs are fetched in CORS
  mode. The upload is a CI job with a scoped token, kept as a repository secret.
- **The licence audit** (`AC-45.4`, GDD 45.9) also fails a pack with a `commercial_ok: false`
  descriptor while OQ-04 is open.
- **Every later gates ADR** (v0.2a onwards) must say, for each panel in its gates, which proxy
  decides it. G2 at v0.3b will be decided by the automated 11-step run (`AC-67.1`) and the owner's
  sorties.
- **Not changed:** the release checklist (§71.6) still requires the external accessibility review
  before 1.0. It is revisited when v0.7b is planned. OQ-11 (G1(a)) stays open.
- **Revisit** the R2 prices and limits when v0.2a's plan is written, and again before the beta.
- **Carried from M1:** the owner's hands-on checklists of v0.1a to v0.1d stay open and do not block.
  They are now one session in `docs/runbooks/reference-machines.md`.
