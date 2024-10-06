// @ts-check
import { defineConfig } from "astro/config";
import starlight from "@astrojs/starlight";

// https://astro.build/config
export default defineConfig({
  site: "https://saidsay-so.github.io",
  base: "/gateau/",
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
    }),
  ],
});
