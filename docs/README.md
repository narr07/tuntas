# Tuntas documentation

Source of https://narr07.github.io/tuntas/, built with the Nuxt UI Docs template and Nuxt Content.

```bash
bun install
bun run dev        # http://localhost:3000/tuntas/
bun run generate   # static site in .output/public
```

Pages live in `content/`. The site is deployed by `.github/workflows/docs.yml` on every push to `main` that touches `docs/`.
