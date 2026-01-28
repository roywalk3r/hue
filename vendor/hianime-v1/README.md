HiAnime Vendor (v1)

Source
- Origin: /home/rseann/projects/Misc/hianime
- Imported: 2026-01-28

Notes
- This is a vendored copy of the HiAnime API server (Hono + Zod OpenAPI).
- Relative ESM imports were normalized to include `.js` extensions for Node ESM.
- Do not edit directly unless updating the vendor snapshot.

Update workflow
1) Replace the `src/` folder with the latest upstream.
2) Run `npm run check:hianime-imports` in the Hue repo.
3) Restart Hue and verify `/api/v1/home`.
