import { createRouter, createWebHashHistory } from "vue-router";
import { dict } from "./dict";

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    { path: "/", redirect: "/fleet" },
    {
      path: "/fleet",
      component: () => import("./views/Fleet/index.vue"),
      meta: { title: dict.nav.fleet },
    },
    {
      path: "/container",
      component: () => import("./views/Container/index.vue"),
      meta: { title: dict.nav.container },
    },
    {
      path: "/image",
      component: () => import("./views/Image/index.vue"),
      meta: { title: dict.nav.image },
    },
    {
      path: "/compose",
      component: () => import("./views/Compose/index.vue"),
      meta: { title: dict.nav.compose },
    },
    {
      path: "/monitor",
      component: () => import("./views/Monitor/index.vue"),
      meta: { title: dict.nav.monitor },
    },
    {
      path: "/setting",
      component: () => import("./views/Setting/index.vue"),
      meta: { title: dict.nav.setting },
    },
  ],
});

export default router;
