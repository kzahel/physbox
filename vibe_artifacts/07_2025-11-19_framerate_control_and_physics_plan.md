# GitHub Pages Setup

## Goal
Automate deployment of the `main` branch build artifacts to GitHub Pages using GitHub Actions.

## Proposed Changes
### Configuration
#### [NEW] vite.config.ts
- Create file with `base: '/physbox/'` configuration.

### CI/CD
#### [NEW] .github/workflows/deploy.yml
- Create a workflow that:
    - Triggers on push to `main`.
    - Sets up Node.js.
    - Installs dependencies (`npm ci`).
    - Builds the project (`npm run build`).
    - Uploads the `dist` folder as an artifact.
    - Deploys to GitHub Pages using `actions/deploy-pages`.

## Verification Plan
- Push changes.
- User checks GitHub Actions tab for success.
- User checks the deployed URL.
