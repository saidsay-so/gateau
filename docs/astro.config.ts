import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";
import { exec as childExec } from "node:child_process";
import { promisify } from "node:util";

const exec = promisify(childExec);

// https://astro.build/config
export default defineConfig({
  site: "https://saidsay-so.github.io",
  base: "/gateau/",
  vite: {
    build: {
      modulePreload: {
        polyfill: false,
      },
    },
    plugins: [
      {
        name: "isolation",
        configureServer(server) {
          server.middlewares.use((_req, res, next) => {
            res.setHeader("Cross-Origin-Opener-Policy", "same-origin");
            res.setHeader("Cross-Origin-Embedder-Policy", "require-corp");
            next();
          });
        },
      },
      // {
      //   name: "cargo:build",
      //   async buildStart(options) {
      //     try {
      //       await exec(
      //         "cargo build --target=wasm32-wasi --features=wasm --manifest-path=../packages/cli/Cargo.toml --release --quiet"
      //       );
      //       console.info("Cargo build succeeded");
      //     } catch (error) {
      //       console.error(error);
      //     }
      //   },
      // },
    ],
  },
  integrations: [
    starlight({
      title: "gateau",
      logo: {
        src: "./src/assets/logo.svg",
      },
      social: {
        github: "https://github.com/saidsay-so/gateau",
      },
      editLink: {
        baseUrl: "https://github.com/saidsay-so/gateau/edit/main/docs",
      },
      sidebar: [
        {
          label: "First Steps",
          autogenerate: { directory: "first-steps" },
        },
      ],
      customCss: [
        // Relative path to your custom CSS file
        "./src/styles/custom.css",
      ],
    }),
  ],
});
